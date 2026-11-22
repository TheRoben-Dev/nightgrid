//! Finding other Nightgrid apps on the local network (D-208).
//!
//! No mDNS daemon, no internet, no tailnet: a device that has "Let my other
//! devices find me" on answers a small UDP probe on [`DISCOVERY_PORT`] with a
//! [`Beacon`] (its name, sync port, version and whether it takes pairing by
//! profile passphrase). A device looking for others sends the probe to the
//! LAN broadcast addresses and collects the answers for a moment.
//!
//! The beacon carries nothing secret. Everything after it (pairing, sync)
//! runs over the authenticated, encrypted channel in [`super::wire`].

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use zeroize::Zeroizing;

/// UDP port a findable device answers probes on (the sync port + 1).
pub const DISCOVERY_PORT: u16 = 47921;
const PROBE: &[u8] = b"NGSD?1";
const ANSWER: &[u8] = b"NGSD!1";
const MAX_PACKET: usize = 1200;
/// PBKDF2 rounds for the passphrase pairing key.
const PASS_ROUNDS: u32 = 210_000;
/// Answers per address and second, so a probe can't be used to flood.
const ANSWER_GAP: Duration = Duration::from_millis(400);

/// What a findable device says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Beacon {
    /// Device id (see [`super::device_id`]).
    pub id: String,
    pub name: String,
    /// TCP port its sync listener uses.
    pub port: u16,
    pub app_version: String,
    pub schema: i64,
    /// Hosts in its profile, so a fresh install can say what it'd import.
    pub hosts: u32,
    /// It takes pairing with the profile passphrase; `salt` goes with it.
    pub passphrase: bool,
    #[serde(default)]
    pub salt: String,
}

/// The probe packet.
pub fn probe_packet() -> &'static [u8] {
    PROBE
}

pub fn is_probe(buf: &[u8]) -> bool {
    buf == PROBE
}

/// The answer packet for a beacon.
pub fn answer_packet(b: &Beacon) -> Vec<u8> {
    let mut out = ANSWER.to_vec();
    out.extend_from_slice(&serde_json::to_vec(b).unwrap_or_default());
    out
}

/// Reads an answer packet; `None` for anything that isn't a sane beacon.
pub fn parse_answer(buf: &[u8]) -> Option<Beacon> {
    if buf.len() > MAX_PACKET || !buf.starts_with(ANSWER) {
        return None;
    }
    let mut b: Beacon = serde_json::from_slice(&buf[ANSWER.len()..]).ok()?;
    let id_ok = !b.id.is_empty() && b.id.len() <= 64 && b.id.bytes().all(|c| c.is_ascii_alphanumeric());
    if !id_ok || b.port == 0 {
        return None;
    }
    b.name = super::clean_name(&b.name);
    b.app_version = b.app_version.chars().filter(|c| c.is_ascii_graphic()).take(32).collect();
    if b.passphrase && decode_salt(&b.salt).is_none() {
        b.passphrase = false;
    }
    Some(b)
}

/// Where to send probes: the limited broadcast, plus the /24 broadcast of
/// the LAN address (macOS sends 255.255.255.255 out of one interface only).
pub fn targets(lan: Option<Ipv4Addr>, port: u16) -> Vec<SocketAddr> {
    let mut v = vec![SocketAddr::from((Ipv4Addr::BROADCAST, port))];
    if let Some(ip) = lan {
        let o = ip.octets();
        let directed = SocketAddr::from((Ipv4Addr::new(o[0], o[1], o[2], 255), port));
        if !v.contains(&directed) {
            v.push(directed);
        }
    }
    v
}

/// A device that answered, with the address it answered from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub addr: IpAddr,
    pub beacon: Beacon,
}

/// Sends probes to `targets` and collects answers for `wait`. `own_id`
/// (this device) is left out; each device is listed once.
pub async fn scan(targets: &[SocketAddr], wait: Duration, own_id: &str) -> io::Result<Vec<Found>> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    sock.set_broadcast(true)?;
    let send = |sock: &UdpSocket| {
        for t in targets {
            // An unreachable target (no route, no broadcast there) isn't fatal.
            let _ = sock.try_send_to(PROBE, *t);
        }
    };
    send(&sock);
    let start = Instant::now();
    let mut resent = false;
    let mut out: Vec<Found> = Vec::new();
    let mut buf = vec![0u8; MAX_PACKET + 1];
    loop {
        let left = wait.saturating_sub(start.elapsed());
        if left.is_zero() {
            break;
        }
        // One more round halfway, in case the first packet was dropped.
        let step = if resent { left } else { left.min(wait / 2) };
        match tokio::time::timeout(step, sock.recv_from(&mut buf)).await {
            Ok(Ok((n, from))) => {
                let Some(b) = parse_answer(&buf[..n]) else { continue };
                if b.id == own_id || out.iter().any(|f| f.beacon.id == b.id) {
                    continue;
                }
                out.push(Found { addr: from.ip(), beacon: b });
            }
            Ok(Err(_)) => continue,
            Err(_) => {
                if !resent {
                    resent = true;
                    send(&sock);
                }
            }
        }
    }
    out.sort_by_key(|f| f.beacon.name.to_lowercase());
    Ok(out)
}

/// Answers probes on `sock` with whatever `beacon` returns (`None`: stay
/// quiet). Runs until the task is dropped.
pub async fn answer(sock: UdpSocket, beacon: impl Fn() -> Option<Beacon>) {
    let mut buf = vec![0u8; 64];
    let mut last: HashMap<IpAddr, Instant> = HashMap::new();
    loop {
        let Ok((n, from)) = sock.recv_from(&mut buf).await else { continue };
        if !is_probe(&buf[..n]) {
            continue;
        }
        let now = Instant::now();
        if last.get(&from.ip()).is_some_and(|t| now.duration_since(*t) < ANSWER_GAP) {
            continue;
        }
        if last.len() > 256 {
            last.retain(|_, t| now.duration_since(*t) < ANSWER_GAP);
        }
        last.insert(from.ip(), now);
        if let Some(b) = beacon() {
            let _ = sock.send_to(&answer_packet(&b), from).await;
        }
    }
}

// ------------------------------------------------------- passphrase pairing

/// A fresh random salt, hex.
pub fn new_salt() -> String {
    use ring::rand::SecureRandom as _;
    let mut b = [0u8; 16];
    ring::rand::SystemRandom::new().fill(&mut b).expect("system random");
    hex(&b)
}

pub fn decode_salt(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The pairing secret both sides derive from the profile passphrase and the
/// findable device's salt. It's slow on purpose; SPAKE2 on top means a
/// listener can't test guesses offline at all.
pub fn pass_key(passphrase: &str, salt: &[u8; 16]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(PASS_ROUNDS).expect("non-zero"),
        salt,
        passphrase.as_bytes(),
        &mut out[..],
    );
    out
}

/// Stored form of the passphrase pairing secret: `salt:key`, hex.
pub fn encode_pass(salt: &str, key: &[u8; 32]) -> String {
    format!("{salt}:{}", hex(key))
}

pub fn decode_pass(s: &str) -> Option<(String, Zeroizing<[u8; 32]>)> {
    let (salt, key) = s.split_once(':')?;
    decode_salt(salt)?;
    if key.len() != 64 {
        return None;
    }
    let mut out = Zeroizing::new([0u8; 32]);
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(key.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some((salt.to_owned(), out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beacon(id: &str) -> Beacon {
        Beacon {
            id: id.into(),
            name: "Desk\ntop".into(),
            port: 47920,
            app_version: "1.27.0".into(),
            schema: 60,
            hosts: 12,
            passphrase: true,
            salt: new_salt(),
        }
    }

    #[test]
    fn packets_round_trip_and_are_checked() {
        let b = beacon("abc123");
        let got = parse_answer(&answer_packet(&b)).unwrap();
        assert_eq!(got.name, "Desktop");
        assert_eq!(got.hosts, 12);
        assert!(got.passphrase);
        assert!(parse_answer(b"NGSD!1{}").is_none());
        assert!(parse_answer(PROBE).is_none());
        let mut bad = beacon("../x");
        assert!(parse_answer(&answer_packet(&bad)).is_none());
        bad = beacon("ok");
        bad.salt = "nope".into();
        assert!(!parse_answer(&answer_packet(&bad)).unwrap().passphrase);
        assert!(is_probe(probe_packet()));
    }

    #[test]
    fn targets_add_the_lan_broadcast() {
        let t = targets(Some(Ipv4Addr::new(192, 168, 1, 20)), 9);
        assert_eq!(t.len(), 2);
        assert_eq!(t[1], "192.168.1.255:9".parse().unwrap());
        assert_eq!(targets(None, 9).len(), 1);
    }

    #[test]
    fn pass_keys_depend_on_salt_and_passphrase() {
        let s1 = decode_salt(&new_salt()).unwrap();
        let s2 = decode_salt(&new_salt()).unwrap();
        assert_eq!(*pass_key("correct horse", &s1), *pass_key("correct horse", &s1));
        assert_ne!(*pass_key("correct horse", &s1), *pass_key("correct horse", &s2));
        assert_ne!(*pass_key("correct horse", &s1), *pass_key("correct house", &s1));
        let salt = new_salt();
        let k = pass_key("x", &decode_salt(&salt).unwrap());
        let (s, k2) = decode_pass(&encode_pass(&salt, &k)).unwrap();
        assert_eq!(s, salt);
        assert_eq!(*k2, *k);
        assert!(decode_pass("zz:00").is_none());
    }

    #[tokio::test]
    async fn finds_a_device_over_loopback() {
        let sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = sock.local_addr().unwrap().port();
        let b = beacon("dev1");
        let b2 = b.clone();
        let task = tokio::spawn(answer(sock, move || Some(b2.clone())));
        let target = [SocketAddr::from((Ipv4Addr::LOCALHOST, port))];
        let found = scan(&target, Duration::from_millis(600), "me").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].beacon.id, "dev1");
        assert_eq!(found[0].addr, IpAddr::from(Ipv4Addr::LOCALHOST));
        // Its own beacon is left out.
        let found = scan(&target, Duration::from_millis(600), "dev1").await.unwrap();
        assert!(found.is_empty());
        task.abort();
    }

    async fn pair_with(typed: &str, stored: &str) -> bool {
        use crate::sync::{pair, wire, Keypair};
        let salt = new_salt();
        let raw = decode_salt(&salt).unwrap();
        let (a, b) = (Keypair::generate().unwrap(), Keypair::generate().unwrap());
        let (ka, kb) = (pass_key(typed, &raw), pass_key(stored, &raw));
        let (mut x, mut y) = tokio::io::duplex(8192);
        let left = async {
            let psk = pair::pass_initiator(&mut x, &ka).await?;
            let mut ch = wire::pair_initiator(x, &a, &psk).await?;
            ch.send_json(&"hi").await?;
            Ok::<_, io::Error>(ch.remote_key())
        };
        let right = async {
            let psk = pair::pass_responder(&mut y, &kb).await?;
            let mut ch = wire::pair_responder(y, &b, &psk).await?;
            let got: String = ch.recv_json().await?;
            Ok::<_, io::Error>((ch.remote_key(), got))
        };
        let (l, r) = tokio::join!(left, right);
        match (l, r) {
            (Ok(kl), Ok((kr, msg))) => kl == b.public && kr == a.public && msg == "hi",
            _ => false,
        }
    }

    #[tokio::test]
    async fn pairs_by_passphrase_end_to_end() {
        assert!(pair_with("correct horse battery", "correct horse battery").await);
        assert!(!pair_with("correct horse batt", "correct horse battery").await);
    }
}

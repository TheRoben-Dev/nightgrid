//! A small read-only SNMP client: v2c (community) and v3 (USM, RFC 3414)
//! with MD5/SHA authentication and DES/AES-128 privacy (RFC 3826). Only GET
//! and GETBULK walks: Nightgrid never SETs anything over SNMP.
//!
//! Used by the SNMP integration as the no-admin way to watch a NAS (D-199).
//! Everything is UDP to one configured address; nothing else is contacted.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::UdpSocket;
use zeroize::Zeroizing;

pub type Oid = Vec<u32>;

/// Parses a dotted OID (`1.3.6.1.2.1.1.1.0`, leading dot allowed).
pub fn oid(s: &str) -> Oid {
    s.trim_start_matches('.').split('.').filter_map(|p| p.parse().ok()).collect()
}

pub fn oid_str(o: &[u32]) -> String {
    o.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    /// Counter32, Gauge32, TimeTicks, Counter64.
    Uint(u64),
    Str(Vec<u8>),
    Oid(Oid),
    Ip([u8; 4]),
    Null,
    /// noSuchObject / noSuchInstance.
    NoSuch,
    EndOfMib,
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Uint(u) => i64::try_from(*u).ok(),
            Value::Str(s) => std::str::from_utf8(s).ok()?.trim().parse().ok(),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Uint(u) => Some(*u as f64),
            Value::Str(s) => std::str::from_utf8(s).ok()?.trim().parse().ok(),
            _ => None,
        }
    }
    pub fn text(&self) -> String {
        match self {
            Value::Str(s) => String::from_utf8_lossy(s).trim_end_matches('\0').trim().to_owned(),
            Value::Int(i) => i.to_string(),
            Value::Uint(u) => u.to_string(),
            Value::Oid(o) => oid_str(o),
            Value::Ip(a) => format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3]),
            _ => String::new(),
        }
    }
}

// ---------- BER ----------

fn push_len(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes: Vec<u8> = len.to_be_bytes().iter().copied().skip_while(|b| *b == 0).collect();
        out.push(0x80 | bytes.len() as u8);
        out.extend(bytes);
    }
}

fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 4);
    out.push(tag);
    push_len(&mut out, body.len());
    out.extend_from_slice(body);
    out
}

fn int(v: i64) -> Vec<u8> {
    let b = v.to_be_bytes();
    let mut start = 0;
    while start < 7 && ((b[start] == 0 && b[start + 1] & 0x80 == 0) || (b[start] == 0xff && b[start + 1] & 0x80 != 0)) {
        start += 1;
    }
    tlv(0x02, &b[start..])
}

fn octets(v: &[u8]) -> Vec<u8> {
    tlv(0x04, v)
}

fn seq(parts: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x30, &parts.concat())
}

fn enc_oid(o: &[u32]) -> Vec<u8> {
    let mut body = Vec::new();
    if o.len() >= 2 {
        body.push((o[0] * 40 + o[1]) as u8);
    }
    for &n in o.iter().skip(2) {
        let mut chunk = vec![(n & 0x7f) as u8];
        let mut v = n >> 7;
        while v > 0 {
            chunk.push(0x80 | (v & 0x7f) as u8);
            v >>= 7;
        }
        chunk.reverse();
        body.extend(chunk);
    }
    tlv(0x06, &body)
}

/// One TLV read from `buf`: tag, contents, and the rest after it.
fn read(buf: &[u8]) -> Result<(u8, &[u8], &[u8]), String> {
    let bad = || "malformed SNMP reply".to_string();
    let (&tag, rest) = buf.split_first().ok_or_else(bad)?;
    let (&l0, rest) = rest.split_first().ok_or_else(bad)?;
    let (len, rest) = if l0 & 0x80 == 0 {
        (l0 as usize, rest)
    } else {
        let n = (l0 & 0x7f) as usize;
        if n == 0 || n > 4 || rest.len() < n {
            return Err(bad());
        }
        (rest[..n].iter().fold(0usize, |a, b| (a << 8) | *b as usize), &rest[n..])
    };
    if rest.len() < len {
        return Err(bad());
    }
    Ok((tag, &rest[..len], &rest[len..]))
}

fn expect(buf: &[u8], want: u8) -> Result<(&[u8], &[u8]), String> {
    let (tag, body, rest) = read(buf)?;
    if tag != want {
        return Err(format!("unexpected SNMP field (tag {tag:#x})"));
    }
    Ok((body, rest))
}

fn dec_int(body: &[u8]) -> i64 {
    if body.is_empty() {
        return 0;
    }
    let mut v: i64 = if body[0] & 0x80 != 0 { -1 } else { 0 };
    for b in body.iter().take(8) {
        v = (v << 8) | *b as i64;
    }
    v
}

fn dec_uint(body: &[u8]) -> u64 {
    body.iter().take(9).fold(0u64, |a, b| (a << 8) | *b as u64)
}

fn dec_oid(body: &[u8]) -> Oid {
    let mut out = Vec::new();
    if let Some(&first) = body.first() {
        out.push((first / 40).min(2) as u32);
        out.push((first as u32).saturating_sub(out[0] * 40));
    }
    let mut v: u32 = 0;
    for &b in body.iter().skip(1) {
        v = (v << 7) | (b & 0x7f) as u32;
        if b & 0x80 == 0 {
            out.push(v);
            v = 0;
        }
    }
    out
}

fn dec_value(tag: u8, body: &[u8]) -> Value {
    match tag {
        0x02 => Value::Int(dec_int(body)),
        0x04 | 0x44 => Value::Str(body.to_vec()),
        0x06 => Value::Oid(dec_oid(body)),
        0x40 if body.len() == 4 => Value::Ip([body[0], body[1], body[2], body[3]]),
        0x41 | 0x42 | 0x43 | 0x46 => Value::Uint(dec_uint(body)),
        0x80 | 0x81 => Value::NoSuch,
        0x82 => Value::EndOfMib,
        _ => Value::Null,
    }
}

const GET: u8 = 0xa0;
const GET_BULK: u8 = 0xa5;
const RESPONSE: u8 = 0xa2;
const REPORT: u8 = 0xa8;

fn pdu(tag: u8, req_id: i32, a: i64, b: i64, oids: &[Oid]) -> Vec<u8> {
    let binds: Vec<Vec<u8>> = oids.iter().map(|o| seq(&[enc_oid(o), tlv(0x05, &[])])).collect();
    tlv(tag, &[int(req_id as i64), int(a), int(b), seq(&binds)].concat())
}

#[derive(Debug, Clone, PartialEq)]
struct Pdu {
    tag: u8,
    req_id: i64,
    error_status: i64,
    error_index: i64,
    binds: Vec<(Oid, Value)>,
}

fn parse_pdu(buf: &[u8]) -> Result<Pdu, String> {
    let (tag, body, _) = read(buf)?;
    let (rid, rest) = expect(body, 0x02)?;
    let (es, rest) = expect(rest, 0x02)?;
    let (ei, rest) = expect(rest, 0x02)?;
    let (mut list, _) = expect(rest, 0x30)?;
    let mut binds = Vec::new();
    while !list.is_empty() {
        let (vb, next) = expect(list, 0x30)?;
        let (o, vrest) = expect(vb, 0x06)?;
        let (vtag, vbody, _) = read(vrest)?;
        binds.push((dec_oid(o), dec_value(vtag, vbody)));
        list = next;
    }
    Ok(Pdu { tag, req_id: dec_int(rid), error_status: dec_int(es), error_index: dec_int(ei), binds })
}

// ---------- USM (v3) ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthProto {
    Md5,
    Sha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivProto {
    Des,
    Aes,
}

fn hash(p: AuthProto, data: &[u8]) -> Vec<u8> {
    use sha1::Digest;
    match p {
        AuthProto::Md5 => md5::compute(data).0.to_vec(),
        AuthProto::Sha => sha1::Sha1::digest(data).to_vec(),
    }
}

fn hmac(p: AuthProto, key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut k = [0u8; 64];
    k[..key.len()].copy_from_slice(key);
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).chain(msg.iter().copied()).collect();
    let inner = hash(p, &ipad);
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).chain(inner).collect();
    hash(p, &opad)
}

/// RFC 3414 A.2: password → localized key for one engine.
pub fn localize_key(p: AuthProto, password: &str, engine_id: &[u8]) -> Zeroizing<Vec<u8>> {
    let pw = password.as_bytes();
    let mut buf = Zeroizing::new(Vec::with_capacity(1_048_576));
    if !pw.is_empty() {
        while buf.len() < 1_048_576 {
            let next = pw[buf.len() % pw.len()];
            buf.push(next);
        }
    }
    let ku = Zeroizing::new(hash(p, &buf));
    let joined = Zeroizing::new([&ku[..], engine_id, &ku[..]].concat());
    Zeroizing::new(hash(p, &joined))
}

fn aes_cfb(key: &[u8], iv: &[u8; 16], data: &mut [u8], decrypt: bool) {
    use aes::cipher::{BlockCipherEncrypt, KeyInit};
    let c = aes::Aes128::new_from_slice(&key[..16]).expect("16-byte key");
    let mut reg = *iv;
    for chunk in data.chunks_mut(16) {
        let mut ks = reg;
        c.encrypt_block((&mut ks).into());
        let mut next = [0u8; 16];
        for (i, b) in chunk.iter_mut().enumerate() {
            let cipher_byte = if decrypt { *b } else { *b ^ ks[i] };
            *b ^= ks[i];
            next[i] = cipher_byte;
        }
        reg = next;
    }
}

fn des_cbc(key: &[u8], iv: &[u8; 8], data: &mut [u8], decrypt: bool) {
    use des::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
    let c = des::Des::new_from_slice(&key[..8]).expect("8-byte key");
    let mut prev = *iv;
    for chunk in data.chunks_exact_mut(8) {
        if decrypt {
            let ct: [u8; 8] = chunk.try_into().expect("8 bytes");
            c.decrypt_block(chunk.try_into().expect("8-byte block"));
            for (b, p) in chunk.iter_mut().zip(prev) {
                *b ^= p;
            }
            prev = ct;
        } else {
            for (b, p) in chunk.iter_mut().zip(prev) {
                *b ^= p;
            }
            c.encrypt_block(chunk.try_into().expect("8-byte block"));
            prev = chunk.try_into().expect("8 bytes");
        }
    }
}

/// How to reach and log in to an SNMP agent.
#[derive(Clone)]
pub enum Security {
    V2c { community: String },
    V3 { user: String, auth: AuthProto, auth_pass: String, privacy: Option<(PrivProto, String)> },
}

impl std::fmt::Debug for Security {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Security::V2c { .. } => f.write_str("V2c"),
            Security::V3 { user, auth, privacy, .. } => {
                write!(f, "V3 {{ user: {user:?}, auth: {auth:?}, privacy: {:?} }}", privacy.as_ref().map(|p| p.0))
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Engine {
    id: Vec<u8>,
    boots: i64,
    time: i64,
}

struct Keys {
    auth: Zeroizing<Vec<u8>>,
    privacy: Option<Zeroizing<Vec<u8>>>,
}

/// A session with one agent.
pub struct Client {
    sock: UdpSocket,
    sec: Security,
    timeout: Duration,
    req_id: i32,
    msg_id: i32,
    salt: u64,
    engine: Engine,
    /// When `engine.time` was learned, to keep it current.
    synced: std::time::Instant,
    keys: Option<Keys>,
}

const USM_ERRORS: [(&str, &str); 6] = [
    ("1.3.6.1.6.3.15.1.1.1.0", "the agent doesn't allow this security level (check the privacy setting)"),
    ("1.3.6.1.6.3.15.1.1.2.0", "the agent's clock window moved"),
    ("1.3.6.1.6.3.15.1.1.3.0", "the agent doesn't know this SNMPv3 user name"),
    ("1.3.6.1.6.3.15.1.1.4.0", "the agent's engine id changed"),
    ("1.3.6.1.6.3.15.1.1.5.0", "wrong SNMPv3 password or authentication protocol"),
    ("1.3.6.1.6.3.15.1.1.6.0", "wrong privacy password or privacy protocol"),
];

impl Client {
    /// Opens a UDP socket to `host:port`. v3 discovers the agent's engine.
    pub async fn connect(host: &str, port: u16, sec: Security, timeout: Duration) -> Result<Self, String> {
        let addr: SocketAddr = tokio::time::timeout(timeout, tokio::net::lookup_host((host, port)))
            .await
            .map_err(|_| format!("couldn't resolve {host}"))?
            .map_err(|e| format!("couldn't resolve {host}: {e}"))?
            .next()
            .ok_or_else(|| format!("couldn't resolve {host}"))?;
        let bind = if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
        let sock = UdpSocket::bind(bind).await.map_err(|e| e.to_string())?;
        sock.connect(addr).await.map_err(|e| e.to_string())?;
        let mut seed = [0u8; 12];
        let _ = getrandom::fill(&mut seed);
        let mut c = Client {
            sock,
            sec,
            timeout,
            req_id: i32::from_be_bytes([seed[0], seed[1], seed[2], seed[3]]) & 0x3fff_ffff,
            msg_id: i32::from_be_bytes([seed[4], seed[5], seed[6], seed[7]]) & 0x3fff_ffff,
            salt: u64::from_be_bytes([seed[8], seed[9], seed[10], seed[11], seed[0], seed[5], seed[2], seed[7]]),
            engine: Engine::default(),
            synced: std::time::Instant::now(),
            keys: None,
        };
        if matches!(c.sec, Security::V3 { .. }) {
            c.discover().await?;
        }
        Ok(c)
    }

    async fn roundtrip(&self, msg: &[u8]) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; 65_535];
        for _attempt in 0..2 {
            self.sock.send(msg).await.map_err(|e| e.to_string())?;
            match tokio::time::timeout(self.timeout, self.sock.recv(&mut buf)).await {
                Ok(Ok(n)) => return Ok(buf[..n].to_vec()),
                Ok(Err(e)) => {
                    return Err(if e.kind() == std::io::ErrorKind::ConnectionRefused {
                        "nothing answers SNMP on that address (is the SNMP service on?)".into()
                    } else {
                        e.to_string()
                    })
                }
                Err(_) => continue,
            }
        }
        Err(match &self.sec {
            Security::V2c { .. } => "no SNMP answer: check the address, that SNMP is on and the community name".into(),
            Security::V3 { .. } => "no SNMP answer: check the address and that the SNMP service is on".into(),
        })
    }

    async fn discover(&mut self) -> Result<(), String> {
        let id = self.next_req();
        let msg = self.v3_message(&pdu(GET, id, 0, 0, &[]), true)?;
        let reply = self.roundtrip(&msg).await?;
        let parsed = parse_v3(&reply)?;
        if parsed.engine.id.is_empty() {
            return Err("the agent didn't send its engine id".into());
        }
        self.engine = parsed.engine;
        self.synced = std::time::Instant::now();
        if let Security::V3 { auth, auth_pass, privacy, .. } = &self.sec {
            if auth_pass.len() < 8 {
                return Err("SNMPv3 passwords are at least 8 characters".into());
            }
            let privacy = privacy.as_ref().map(|(_, p)| localize_key(*auth, p, &self.engine.id));
            self.keys = Some(Keys { auth: localize_key(*auth, auth_pass, &self.engine.id), privacy });
        }
        Ok(())
    }

    fn next_req(&mut self) -> i32 {
        self.req_id = (self.req_id + 1) & 0x3fff_ffff;
        self.req_id
    }

    /// A v3 message around `pdu`. `discovery`: noAuth, empty user.
    fn v3_message(&mut self, pdu: &[u8], discovery: bool) -> Result<Vec<u8>, String> {
        let Security::V3 { user, auth, privacy, .. } = &self.sec else { return Err("not v3".into()) };
        self.msg_id = (self.msg_id + 1) & 0x3fff_ffff;
        let time = if discovery { 0 } else { self.engine.time + self.synced.elapsed().as_secs() as i64 };
        let scoped = seq(&[octets(&self.engine.id), octets(b""), pdu.to_vec()]);
        let (flags, user, auth_params, priv_params, data) = if discovery {
            (0x04u8, &b""[..], Vec::new(), Vec::new(), scoped)
        } else {
            let keys = self.keys.as_ref().ok_or("no SNMPv3 keys")?;
            match (privacy, &keys.privacy) {
                (Some((proto, _)), Some(pk)) => {
                    self.salt = self.salt.wrapping_add(1);
                    let mut data = scoped;
                    let salt: [u8; 8];
                    match proto {
                        PrivProto::Aes => {
                            salt = self.salt.to_be_bytes();
                            let mut iv = [0u8; 16];
                            iv[..4].copy_from_slice(&(self.engine.boots as u32).to_be_bytes());
                            iv[4..8].copy_from_slice(&(time as u32).to_be_bytes());
                            iv[8..].copy_from_slice(&salt);
                            aes_cfb(pk, &iv, &mut data, false);
                        }
                        PrivProto::Des => {
                            let mut s = [0u8; 8];
                            s[..4].copy_from_slice(&(self.engine.boots as u32).to_be_bytes());
                            s[4..].copy_from_slice(&(self.salt as u32).to_be_bytes());
                            salt = s;
                            let mut iv = [0u8; 8];
                            for (i, b) in iv.iter_mut().enumerate() {
                                *b = pk[8 + i] ^ salt[i];
                            }
                            while data.len() % 8 != 0 {
                                data.push(0);
                            }
                            des_cbc(pk, &iv, &mut data, false);
                        }
                    }
                    (0x07, user.as_bytes(), vec![0u8; 12], salt.to_vec(), octets(&data))
                }
                _ => (0x05, user.as_bytes(), vec![0u8; 12], Vec::new(), scoped),
            }
        };
        let usm = seq(&[
            octets(&self.engine.id),
            int(self.engine.boots),
            int(time),
            octets(user),
            octets(&auth_params),
            octets(&priv_params),
        ]);
        let header = seq(&[int(self.msg_id as i64), int(65_507), octets(&[flags]), int(3)]);
        let mut msg = seq(&[int(3), header, octets(&usm), data]);
        if flags & 0x01 != 0 {
            let keys = self.keys.as_ref().ok_or("no SNMPv3 keys")?;
            let mac = hmac(*auth, &keys.auth, &msg);
            // The zeroed authParams sit at a known place: find the 12 zero
            // bytes right after "04 0c" inside the USM block.
            let pos = find_auth_params(&msg)?;
            msg[pos..pos + 12].copy_from_slice(&mac[..12]);
        }
        Ok(msg)
    }

    async fn request(&mut self, tag: u8, a: i64, b: i64, oids: &[Oid]) -> Result<Pdu, String> {
        for attempt in 0..2 {
            let id = self.next_req();
            let p = pdu(tag, id, a, b, oids);
            let msg = match &self.sec {
                Security::V2c { community } => seq(&[int(1), octets(community.as_bytes()), p]),
                Security::V3 { .. } => self.v3_message(&p, false)?,
            };
            let reply = self.roundtrip(&msg).await?;
            let pdu = match &self.sec {
                Security::V2c { .. } => {
                    let (body, _) = expect(&reply, 0x30)?;
                    let (_, rest) = expect(body, 0x02)?;
                    let (_, rest) = expect(rest, 0x04)?;
                    parse_pdu(rest)?
                }
                Security::V3 { auth, privacy, .. } => {
                    let parsed = parse_v3(&reply)?;
                    let keys = self.keys.as_ref().ok_or("no SNMPv3 keys")?;
                    if parsed.flags & 0x01 != 0 {
                        let mut zeroed = reply.clone();
                        let pos = parsed.auth_pos;
                        if parsed.auth_params.len() != 12 {
                            return Err("bad SNMPv3 authentication field".into());
                        }
                        zeroed[pos..pos + 12].fill(0);
                        let mac = hmac(*auth, &keys.auth, &zeroed);
                        if mac[..12] != parsed.auth_params[..] {
                            return Err("the SNMP reply failed authentication".into());
                        }
                    }
                    let scoped = if parsed.flags & 0x02 != 0 {
                        let (Some((proto, _)), Some(pk)) = (privacy, &keys.privacy) else {
                            return Err("the agent encrypted its reply but no privacy password is set".into());
                        };
                        let mut data = parsed.data.clone();
                        let salt = &parsed.priv_params;
                        if salt.len() != 8 {
                            return Err("bad SNMPv3 privacy field".into());
                        }
                        match proto {
                            PrivProto::Aes => {
                                let mut iv = [0u8; 16];
                                iv[..4].copy_from_slice(&(parsed.engine.boots as u32).to_be_bytes());
                                iv[4..8].copy_from_slice(&(parsed.engine.time as u32).to_be_bytes());
                                iv[8..].copy_from_slice(salt);
                                aes_cfb(pk, &iv, &mut data, true);
                            }
                            PrivProto::Des => {
                                if data.len() % 8 != 0 {
                                    return Err("bad SNMPv3 encrypted data".into());
                                }
                                let mut iv = [0u8; 8];
                                for (i, b) in iv.iter_mut().enumerate() {
                                    *b = pk[8 + i] ^ salt[i];
                                }
                                des_cbc(pk, &iv, &mut data, true);
                            }
                        }
                        data
                    } else {
                        parsed.data.clone()
                    };
                    let (body, _) = expect(&scoped, 0x30).map_err(|_| {
                        "couldn't decrypt the SNMP reply: wrong privacy password or protocol".to_string()
                    })?;
                    let (_, rest) = expect(body, 0x04)?;
                    let (_, rest) = expect(rest, 0x04)?;
                    let pdu = parse_pdu(rest)?;
                    if pdu.tag == REPORT {
                        let first = pdu.binds.first().map(|(o, _)| oid_str(o)).unwrap_or_default();
                        if first == USM_ERRORS[1].0 && attempt == 0 {
                            self.engine.boots = parsed.engine.boots;
                            self.engine.time = parsed.engine.time;
                            self.synced = std::time::Instant::now();
                            continue;
                        }
                        let why = USM_ERRORS
                            .iter()
                            .find(|(o, _)| *o == first)
                            .map(|(_, w)| *w)
                            .unwrap_or("the agent refused the request");
                        return Err(format!("SNMP: {why}"));
                    }
                    pdu
                }
            };
            if pdu.tag == REPORT {
                return Err("SNMP: the agent refused the request".into());
            }
            if pdu.req_id != id as i64 {
                // A late answer to an earlier try: ask again.
                continue;
            }
            if pdu.tag != RESPONSE {
                return Err("unexpected SNMP reply".into());
            }
            if pdu.error_status != 0 {
                return Err(match pdu.error_status {
                    2 => "SNMP: no such name".into(),
                    16 => "SNMP: access denied (check the community or user)".into(),
                    n => format!("SNMP error {n} (index {})", pdu.error_index),
                });
            }
            return Ok(pdu);
        }
        Err("SNMP: the agent's clock window moved twice".into())
    }

    /// GET of several scalars. Missing ones come back as [`Value::NoSuch`].
    pub async fn get(&mut self, oids: &[Oid]) -> Result<Vec<(Oid, Value)>, String> {
        Ok(self.request(GET, 0, 0, oids).await?.binds)
    }

    /// Every (oid, value) under `root`, with GETBULK. Stops after `cap` rows.
    pub async fn walk(&mut self, root: &[u32], cap: usize) -> Result<Vec<(Oid, Value)>, String> {
        let mut out = Vec::new();
        let mut cur: Oid = root.to_vec();
        while out.len() < cap {
            let pdu = self.request(GET_BULK, 0, 25, std::slice::from_ref(&cur)).await?;
            if pdu.binds.is_empty() {
                break;
            }
            let mut done = false;
            for (o, v) in pdu.binds {
                if !o.starts_with(root) || matches!(v, Value::EndOfMib) || o <= cur {
                    done = true;
                    break;
                }
                cur = o.clone();
                out.push((o, v));
            }
            if done {
                break;
            }
        }
        Ok(out)
    }
}

fn find_auth_params(msg: &[u8]) -> Result<usize, String> {
    // msg = SEQ { INT 3, header SEQ, OCTET usm, data }
    let base = msg.as_ptr() as usize;
    let (body, _) = expect(msg, 0x30)?;
    let (_, rest) = expect(body, 0x02)?;
    let (_, rest) = expect(rest, 0x30)?;
    let (usm, _) = expect(rest, 0x04)?;
    let (usm_body, _) = expect(usm, 0x30)?;
    let (_, r) = expect(usm_body, 0x04)?;
    let (_, r) = expect(r, 0x02)?;
    let (_, r) = expect(r, 0x02)?;
    let (_, r) = expect(r, 0x04)?;
    let (auth, _) = expect(r, 0x04)?;
    if auth.len() != 12 {
        return Err("bad SNMPv3 authentication field".into());
    }
    Ok(auth.as_ptr() as usize - base)
}

struct V3Reply {
    flags: u8,
    engine: Engine,
    auth_params: Vec<u8>,
    auth_pos: usize,
    priv_params: Vec<u8>,
    /// The scoped PDU, still encrypted when flags has privacy.
    data: Vec<u8>,
}

fn parse_v3(msg: &[u8]) -> Result<V3Reply, String> {
    let base = msg.as_ptr() as usize;
    let (body, _) = expect(msg, 0x30)?;
    let (ver, rest) = expect(body, 0x02)?;
    if dec_int(ver) != 3 {
        return Err("the agent answered with another SNMP version".into());
    }
    let (header, rest) = expect(rest, 0x30)?;
    let (_, h) = expect(header, 0x02)?;
    let (_, h) = expect(h, 0x02)?;
    let (flags, _) = expect(h, 0x04)?;
    let flags = flags.first().copied().unwrap_or(0);
    let (usm, rest) = expect(rest, 0x04)?;
    let (usm_body, _) = expect(usm, 0x30)?;
    let (eid, r) = expect(usm_body, 0x04)?;
    let (boots, r) = expect(r, 0x02)?;
    let (time, r) = expect(r, 0x02)?;
    let (_user, r) = expect(r, 0x04)?;
    let (auth, r) = expect(r, 0x04)?;
    let (privp, _) = expect(r, 0x04)?;
    let (tag, data, _) = read(rest)?;
    let data = if tag == 0x04 { data.to_vec() } else { rest[..rest.len() - read(rest)?.2.len()].to_vec() };
    Ok(V3Reply {
        flags,
        engine: Engine { id: eid.to_vec(), boots: dec_int(boots), time: dec_int(time) },
        auth_params: auth.to_vec(),
        auth_pos: auth.as_ptr() as usize - base,
        priv_params: privp.to_vec(),
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn rfc3414_key_localization() {
        // RFC 3414 A.3.1 / A.3.2: "maplesyrup" with engine 00..02.
        let engine = hex("000000000000000000000002");
        assert_eq!(*localize_key(AuthProto::Md5, "maplesyrup", &engine), hex("526f5eed9fcce26f8964c2930787d82b"));
        assert_eq!(
            *localize_key(AuthProto::Sha, "maplesyrup", &engine),
            hex("6695febc9288e36282235fc7151f128497b38f3f")
        );
    }

    #[test]
    fn hmac_matches_rfc2202() {
        let mac = hmac(AuthProto::Md5, b"Jefe", b"what do ya want for nothing?");
        assert_eq!(mac, hex("750c783e6ab0b503eaa86e310a5db738"));
        let mac = hmac(AuthProto::Sha, b"Jefe", b"what do ya want for nothing?");
        assert_eq!(mac, hex("effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"));
    }

    #[test]
    fn ber_round_trips() {
        assert_eq!(int(0), vec![2, 1, 0]);
        assert_eq!(int(128), vec![2, 2, 0, 128]);
        assert_eq!(int(-1), vec![2, 1, 0xff]);
        assert_eq!(dec_int(&[0, 128]), 128);
        assert_eq!(dec_int(&[0xff]), -1);
        let o = oid("1.3.6.1.4.1.6574.2.1.1.5.300");
        let e = enc_oid(&o);
        let (tag, body, _) = read(&e).unwrap();
        assert_eq!(tag, 6);
        assert_eq!(dec_oid(body), o);
        let long = tlv(0x04, &[7u8; 300]);
        let (_, body, rest) = read(&long).unwrap();
        assert_eq!(body.len(), 300);
        assert!(rest.is_empty());
    }

    #[test]
    fn pdu_parses() {
        let binds = seq(&[
            seq(&[enc_oid(&oid("1.3.6.1.2.1.1.5.0")), octets(b"nas")]),
            seq(&[enc_oid(&oid("1.3.6.1.4.1.6574.3.1.1.5.0")), tlv(0x46, &[1, 0, 0, 0, 0])]),
            seq(&[enc_oid(&oid("1.3.6.1.2.1.1.9.0")), tlv(0x81, &[])]),
        ]);
        let p = tlv(RESPONSE, &[int(42), int(0), int(0), binds].concat());
        let pdu = parse_pdu(&p).unwrap();
        assert_eq!(pdu.req_id, 42);
        assert_eq!(pdu.binds[0].1.text(), "nas");
        assert_eq!(pdu.binds[1].1, Value::Uint(1 << 32));
        assert_eq!(pdu.binds[2].1, Value::NoSuch);
    }

    #[test]
    fn aes_cfb_matches_nist() {
        // SP 800-38A F.3.13, CFB128-AES128, first block.
        let mut d = hex("6bc1bee22e409f96e93d7e117393172a");
        let iv: [u8; 16] = hex("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
        aes_cfb(&hex("2b7e151628aed2a6abf7158809cf4f3c"), &iv, &mut d, false);
        assert_eq!(d, hex("3b3fd92eb72dad20333449f8e83cfb4a"));
    }

    #[test]
    fn ciphers_round_trip() {
        let key = [7u8; 20];
        let plain: Vec<u8> = (0..37u8).collect();
        let mut d = plain.clone();
        aes_cfb(&key, &[1; 16], &mut d, false);
        assert_ne!(d, plain);
        aes_cfb(&key, &[1; 16], &mut d, true);
        assert_eq!(d, plain);
        let mut d: Vec<u8> = (0..40u8).collect();
        des_cbc(&key, &[3; 8], &mut d, false);
        des_cbc(&key, &[3; 8], &mut d, true);
        assert_eq!(d, (0..40u8).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn v3_message_auth_is_placed_and_verifiable() {
        let mut c = Client {
            sock: UdpSocket::bind("127.0.0.1:0").await.unwrap(),
            sec: Security::V3 {
                user: "nightgrid".into(),
                auth: AuthProto::Sha,
                auth_pass: "maplesyrup".into(),
                privacy: Some((PrivProto::Aes, "maplesyrup".into())),
            },
            timeout: Duration::from_millis(10),
            req_id: 1,
            msg_id: 1,
            salt: 9,
            engine: Engine { id: hex("000000000000000000000002"), boots: 3, time: 100 },
            synced: std::time::Instant::now(),
            keys: None,
        };
        c.keys = Some(Keys {
            auth: localize_key(AuthProto::Sha, "maplesyrup", &c.engine.id),
            privacy: Some(localize_key(AuthProto::Sha, "maplesyrup", &c.engine.id)),
        });
        let p = pdu(GET, 5, 0, 0, &[oid("1.3.6.1.2.1.1.5.0")]);
        let msg = c.v3_message(&p, false).unwrap();
        let parsed = parse_v3(&msg).unwrap();
        assert_eq!(parsed.flags, 7);
        let mut zeroed = msg.clone();
        zeroed[parsed.auth_pos..parsed.auth_pos + 12].fill(0);
        let mac = hmac(AuthProto::Sha, &c.keys.as_ref().unwrap().auth, &zeroed);
        assert_eq!(&mac[..12], &parsed.auth_params[..]);
        let mut data = parsed.data.clone();
        let mut iv = [0u8; 16];
        iv[..4].copy_from_slice(&3u32.to_be_bytes());
        iv[4..8].copy_from_slice(&100u32.to_be_bytes());
        iv[8..].copy_from_slice(&parsed.priv_params);
        aes_cfb(&c.keys.as_ref().unwrap().privacy.as_ref().unwrap()[..], &iv, &mut data, true);
        let (body, _) = expect(&data, 0x30).unwrap();
        let (_, rest) = expect(body, 0x04).unwrap();
        let (_, rest) = expect(rest, 0x04).unwrap();
        assert_eq!(parse_pdu(rest).unwrap().req_id, 5);
    }

    /// Against a real net-snmp agent: `NG_SNMP_TEST=127.0.0.1:16161 cargo test -p nightgrid-core snmp::tests::live -- --ignored`
    /// with community `public123`, users shauser (SHA/AES), md5user (MD5/DES),
    /// authonly (SHA), passwords authpass123 / privpass123.
    #[tokio::test]
    #[ignore]
    async fn live_agent() {
        let Ok(addr) = std::env::var("NG_SNMP_TEST") else { return };
        let (host, port) = addr.rsplit_once(':').unwrap();
        let port: u16 = port.parse().unwrap();
        let t = Duration::from_secs(2);
        let v3 = |user: &str, auth: AuthProto, privacy: Option<PrivProto>| Security::V3 {
            user: user.into(),
            auth,
            auth_pass: "authpass123".into(),
            privacy: privacy.map(|p| (p, "privpass123".to_string())),
        };
        let cases = [
            Security::V2c { community: "public123".into() },
            v3("shauser", AuthProto::Sha, Some(PrivProto::Aes)),
            v3("md5user", AuthProto::Md5, Some(PrivProto::Des)),
            v3("authonly", AuthProto::Sha, None),
        ];
        for sec in cases {
            let label = format!("{sec:?}");
            let mut c = Client::connect(host, port, sec, t).await.unwrap_or_else(|e| panic!("{label}: {e}"));
            let got = c.get(&[oid("1.3.6.1.2.1.1.5.0")]).await.unwrap_or_else(|e| panic!("{label}: {e}"));
            assert_eq!(got[0].1.text(), "testnas", "{label}");
            let rows = c.walk(&oid("1.3.6.1.2.1.2.2.1.2"), 500).await.unwrap_or_else(|e| panic!("{label}: {e}"));
            assert!(!rows.is_empty(), "{label}");
        }
        let mut bad =
            Client::connect(host, port, v3("shauser", AuthProto::Sha, Some(PrivProto::Des)), t).await.unwrap();
        assert!(bad.get(&[oid("1.3.6.1.2.1.1.5.0")]).await.is_err());
        let wrong = Security::V3 {
            user: "shauser".into(),
            auth: AuthProto::Sha,
            auth_pass: "wrongpass1".into(),
            privacy: None,
        };
        let e =
            Client::connect(host, port, wrong, t).await.unwrap().get(&[oid("1.3.6.1.2.1.1.5.0")]).await.unwrap_err();
        assert!(e.contains("password") || e.contains("security level"), "{e}");
    }
}

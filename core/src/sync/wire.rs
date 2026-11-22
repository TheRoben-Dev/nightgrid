//! The encrypted channel between two paired apps (D-205).
//!
//! Every connection starts with [`MAGIC`] and one kind byte, then a Noise
//! handshake:
//!
//! - **Pairing** (`P`): a SPAKE2 exchange on the one-time code first (see
//!   [`super::pair`]), then `Noise_XXpsk3` with the SPAKE2 key as the
//!   pre-shared key. Both sides learn each other's static key, and a wrong
//!   code (or anyone in the middle without it) fails the handshake. An
//!   attacker gets one guess per attempt, never an offline brute force.
//! - **Session** (`S`): `Noise_IK` with the keys pinned at pairing. The
//!   responder drops the connection before answering when the initiator's
//!   key isn't one of its paired devices.
//!
//! On the wire each Noise message is a big-endian `u16` length and the
//! message. An application message is split into chunks whose first
//! plaintext byte says whether more follow.

use std::io;

use serde::de::DeserializeOwned;
use serde::Serialize;
use snow::{Builder, HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

pub const MAGIC: &[u8; 4] = b"NGS1";
pub const KIND_PAIR: u8 = b'P';
pub const KIND_SESSION: u8 = b'S';
/// Pairing with the profile passphrase instead of a code (D-208).
pub const KIND_PAIR_PASS: u8 = b'Q';
/// "Show me a pairing code": a device found on the LAN asks this one to
/// put a code on its screen (D-208). Plain bytes: a name, then one answer byte.
pub const KIND_ASK: u8 = b'A';

const PAIR_PATTERN: &str = "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s";
const SESSION_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
const MAX_NOISE: usize = 65535;
const TAG: usize = 16;
const CHUNK: usize = MAX_NOISE - TAG - 1;
/// Largest application message (a profile is a few hundred KB).
pub const MAX_MESSAGE: usize = 64 << 20;

/// A device's long-term X25519 key pair.
pub struct Keypair {
    pub private: Zeroizing<[u8; 32]>,
    pub public: [u8; 32],
}

impl Keypair {
    pub fn generate() -> io::Result<Self> {
        let kp = Builder::new(SESSION_PATTERN.parse().map_err(io::Error::other)?)
            .generate_keypair()
            .map_err(io::Error::other)?;
        Self::from_private(&kp.private)
    }

    /// Rebuilds the pair from the private key (the public key is derived).
    pub fn from_private(private: &[u8]) -> io::Result<Self> {
        let p: [u8; 32] = private.try_into().map_err(|_| io::Error::other("bad key length"))?;
        let public = public_of(&p)?;
        Ok(Self { private: Zeroizing::new(p), public })
    }
}

/// X25519 public key of a private key, through snow's resolver.
fn public_of(private: &[u8; 32]) -> io::Result<[u8; 32]> {
    use snow::params::DHChoice;
    use snow::resolvers::{CryptoResolver, DefaultResolver};
    let mut dh = DefaultResolver.resolve_dh(&DHChoice::Curve25519).ok_or_else(|| io::Error::other("no X25519"))?;
    dh.set(private);
    let mut out = [0u8; 32];
    out.copy_from_slice(dh.pubkey());
    Ok(out)
}

fn noise_err(e: snow::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("secure channel: {e}"))
}

async fn write_frame<S: AsyncWrite + Unpin>(s: &mut S, buf: &[u8]) -> io::Result<()> {
    let len = u16::try_from(buf.len()).map_err(|_| io::Error::other("frame too big"))?;
    let mut out = Vec::with_capacity(buf.len() + 2);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(buf);
    s.write_all(&out).await?;
    s.flush().await
}

async fn read_frame<S: AsyncRead + Unpin>(s: &mut S) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 2];
    s.read_exact(&mut len).await?;
    let mut buf = vec![0u8; usize::from(u16::from_be_bytes(len))];
    s.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Writes the connection header (magic and kind).
pub async fn write_header<S: AsyncWrite + Unpin>(s: &mut S, kind: u8) -> io::Result<()> {
    let mut h = MAGIC.to_vec();
    h.push(kind);
    s.write_all(&h).await
}

/// Reads the connection header; returns the kind.
pub async fn read_header<S: AsyncRead + Unpin>(s: &mut S) -> io::Result<u8> {
    let mut h = [0u8; 5];
    s.read_exact(&mut h).await?;
    if &h[..4] != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a Nightgrid sync connection"));
    }
    Ok(h[4])
}

async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    mut hs: HandshakeState,
    mut check: impl FnMut(&[u8; 32]) -> bool,
) -> io::Result<(TransportState, [u8; 32])> {
    let mut buf = vec![0u8; MAX_NOISE];
    let mut payload = vec![0u8; MAX_NOISE];
    let mut checked = false;
    while !hs.is_handshake_finished() {
        if hs.is_my_turn() {
            let n = hs.write_message(&[], &mut buf).map_err(noise_err)?;
            write_frame(s, &buf[..n]).await?;
        } else {
            let msg = read_frame(s).await?;
            hs.read_message(&msg, &mut payload).map_err(noise_err)?;
            if !checked {
                if let Some(rs) = hs.get_remote_static() {
                    let k: [u8; 32] = rs.try_into().map_err(|_| io::Error::other("bad key"))?;
                    if !check(&k) {
                        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "this device isn't paired"));
                    }
                    checked = true;
                }
            }
        }
    }
    let remote: [u8; 32] = hs
        .get_remote_static()
        .ok_or_else(|| io::Error::other("no remote key"))?
        .try_into()
        .map_err(|_| io::Error::other("bad key"))?;
    Ok((hs.into_transport_mode().map_err(noise_err)?, remote))
}

/// Initiator side of the pairing handshake. `psk` comes from SPAKE2.
pub async fn pair_initiator<S: AsyncRead + AsyncWrite + Unpin>(
    mut s: S,
    local: &Keypair,
    psk: &[u8; 32],
) -> io::Result<Channel<S>> {
    let hs = Builder::new(PAIR_PATTERN.parse().map_err(io::Error::other)?)
        .prologue(b"nightgrid-sync-pair")
        .and_then(|b| b.local_private_key(&local.private[..]))
        .and_then(|b| b.psk(3, psk))
        .and_then(|b| b.build_initiator())
        .map_err(noise_err)?;
    let (t, remote) = handshake(&mut s, hs, |_| true).await?;
    Ok(Channel { s, t, remote })
}

pub async fn pair_responder<S: AsyncRead + AsyncWrite + Unpin>(
    mut s: S,
    local: &Keypair,
    psk: &[u8; 32],
) -> io::Result<Channel<S>> {
    let hs = Builder::new(PAIR_PATTERN.parse().map_err(io::Error::other)?)
        .prologue(b"nightgrid-sync-pair")
        .and_then(|b| b.local_private_key(&local.private[..]))
        .and_then(|b| b.psk(3, psk))
        .and_then(|b| b.build_responder())
        .map_err(noise_err)?;
    let (t, remote) = handshake(&mut s, hs, |_| true).await?;
    Ok(Channel { s, t, remote })
}

/// Opens a session to a paired device whose key is `remote`.
pub async fn session_initiator<S: AsyncRead + AsyncWrite + Unpin>(
    mut s: S,
    local: &Keypair,
    remote: &[u8; 32],
) -> io::Result<Channel<S>> {
    let hs = Builder::new(SESSION_PATTERN.parse().map_err(io::Error::other)?)
        .prologue(b"nightgrid-sync-session")
        .and_then(|b| b.local_private_key(&local.private[..]))
        .and_then(|b| b.remote_public_key(remote))
        .and_then(|b| b.build_initiator())
        .map_err(noise_err)?;
    let (t, got) = handshake(&mut s, hs, |k| k == remote).await?;
    Ok(Channel { s, t, remote: got })
}

/// Accepts a session; `known` says whether a key belongs to a paired device.
pub async fn session_responder<S: AsyncRead + AsyncWrite + Unpin>(
    mut s: S,
    local: &Keypair,
    known: impl FnMut(&[u8; 32]) -> bool,
) -> io::Result<Channel<S>> {
    let hs = Builder::new(SESSION_PATTERN.parse().map_err(io::Error::other)?)
        .prologue(b"nightgrid-sync-session")
        .and_then(|b| b.local_private_key(&local.private[..]))
        .and_then(|b| b.build_responder())
        .map_err(noise_err)?;
    let (t, remote) = handshake(&mut s, hs, known).await?;
    Ok(Channel { s, t, remote })
}

/// An established, encrypted and authenticated connection.
pub struct Channel<S> {
    s: S,
    t: TransportState,
    remote: [u8; 32],
}

impl<S: AsyncRead + AsyncWrite + Unpin> Channel<S> {
    /// The other side's static public key.
    pub fn remote_key(&self) -> [u8; 32] {
        self.remote
    }

    pub async fn send(&mut self, msg: &[u8]) -> io::Result<()> {
        if msg.len() > MAX_MESSAGE {
            return Err(io::Error::other("message too big"));
        }
        let mut out = vec![0u8; MAX_NOISE];
        let mut plain = Zeroizing::new(Vec::with_capacity(CHUNK + 1));
        let mut chunks = msg.chunks(CHUNK).peekable();
        if msg.is_empty() {
            let n = self.t.write_message(&[1], &mut out).map_err(noise_err)?;
            return write_frame(&mut self.s, &out[..n]).await;
        }
        while let Some(c) = chunks.next() {
            plain.clear();
            plain.push(u8::from(chunks.peek().is_none()));
            plain.extend_from_slice(c);
            let n = self.t.write_message(&plain, &mut out).map_err(noise_err)?;
            write_frame(&mut self.s, &out[..n]).await?;
        }
        Ok(())
    }

    pub async fn recv(&mut self) -> io::Result<Zeroizing<Vec<u8>>> {
        let mut msg = Zeroizing::new(Vec::new());
        let mut plain = Zeroizing::new(vec![0u8; MAX_NOISE]);
        loop {
            let f = read_frame(&mut self.s).await?;
            let n = self.t.read_message(&f, &mut plain).map_err(noise_err)?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "empty chunk"));
            }
            if msg.len() + n - 1 > MAX_MESSAGE {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "message too big"));
            }
            msg.extend_from_slice(&plain[1..n]);
            if plain[0] == 1 {
                return Ok(msg);
            }
        }
    }

    pub async fn send_json<T: Serialize>(&mut self, v: &T) -> io::Result<()> {
        let b = Zeroizing::new(serde_json::to_vec(v).map_err(io::Error::other)?);
        self.send(&b).await
    }

    pub async fn recv_json<T: DeserializeOwned>(&mut self) -> io::Result<T> {
        let b = self.recv().await?;
        serde_json::from_slice(&b).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad message: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pairing_needs_the_same_psk() {
        let (a, b) = (Keypair::generate().unwrap(), Keypair::generate().unwrap());
        let (x, y) = tokio::io::duplex(1 << 20);
        let psk = [7u8; 32];
        let (ca, cb) = tokio::join!(pair_initiator(x, &a, &psk), pair_responder(y, &b, &psk));
        let (mut ca, mut cb) = (ca.unwrap(), cb.unwrap());
        assert_eq!(ca.remote_key(), b.public);
        assert_eq!(cb.remote_key(), a.public);
        let big = vec![42u8; 300_000];
        let (r1, r2) = tokio::join!(ca.send(&big), cb.recv());
        r1.unwrap();
        assert_eq!(&r2.unwrap()[..], &big[..]);
        let (r1, r2) = tokio::join!(cb.send(b""), ca.recv());
        r1.unwrap();
        assert!(r2.unwrap().is_empty());

        let (x, y) = tokio::io::duplex(1 << 20);
        let (ca, cb) = tokio::join!(pair_initiator(x, &a, &psk), pair_responder(y, &b, &[8u8; 32]));
        assert!(ca.is_err() || cb.is_err(), "a wrong code must not pair");
    }

    #[tokio::test]
    async fn sessions_only_for_paired_keys() {
        let (a, b, eve) = (Keypair::generate().unwrap(), Keypair::generate().unwrap(), Keypair::generate().unwrap());
        let (x, y) = tokio::io::duplex(1 << 16);
        let a_pub = a.public;
        let (ca, cb) = tokio::join!(session_initiator(x, &a, &b.public), session_responder(y, &b, |k| *k == a_pub));
        let (mut ca, mut cb) = (ca.unwrap(), cb.unwrap());
        let hello = serde_json::json!({ "op": "hello" });
        let (r1, r2) = tokio::join!(ca.send_json(&hello), cb.recv_json::<serde_json::Value>());
        r1.unwrap();
        assert_eq!(r2.unwrap()["op"], "hello");

        // An unpaired key is refused.
        let (x, y) = tokio::io::duplex(1 << 16);
        let (ce, cb) = tokio::join!(session_initiator(x, &eve, &b.public), session_responder(y, &b, |k| *k == a_pub));
        assert!(cb.is_err());
        assert!(ce.is_err());

        // Talking to the wrong responder key fails too.
        let (x, y) = tokio::io::duplex(1 << 16);
        let (ca, ce) = tokio::join!(session_initiator(x, &a, &b.public), session_responder(y, &eve, |_| true));
        assert!(ca.is_err() || ce.is_err());
    }

    #[test]
    fn keys_round_trip() {
        let k = Keypair::generate().unwrap();
        let again = Keypair::from_private(&k.private[..]).unwrap();
        assert_eq!(again.public, k.public);
    }
}

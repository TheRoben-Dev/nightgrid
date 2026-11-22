//! One-time pairing codes and the SPAKE2 step in front of the pairing
//! handshake (D-205).
//!
//! The code is 10 characters of Crockford base32 (50 bits), shown as
//! `ABCDE-FGHJK`. SPAKE2 turns it into a 32-byte key without sending it:
//! someone listening learns nothing, and someone in the middle gets one
//! guess per attempt. The app showing the code allows a few attempts and
//! expires it after a few minutes.

use std::io;

use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

/// Crockford base32: no I, L, O or U.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const CODE_LEN: usize = 10;
/// Seconds a code stays valid.
pub const CODE_TTL_SECS: u64 = 300;
/// Wrong tries before a code is thrown away.
pub const CODE_ATTEMPTS: u32 = 3;
const SPAKE_MSG: usize = 33;
const ID: &[u8] = b"nightgrid-sync-pair-v1";
/// Pairing with the profile passphrase (D-208) uses its own identity, so a
/// code and a passphrase key never agree by accident.
const ID_PASS: &[u8] = b"nightgrid-sync-pass-v1";
/// Wrong passphrases a findable device takes before it stops taking them
/// for [`PASS_LOCKOUT_SECS`].
pub const PASS_ATTEMPTS: u32 = 5;
pub const PASS_LOCKOUT_SECS: u64 = 600;

/// A fresh code, formatted `ABCDE-FGHJK`.
pub fn new_code() -> String {
    use ring::rand::SecureRandom as _;
    let mut b = [0u8; CODE_LEN];
    ring::rand::SystemRandom::new().fill(&mut b).expect("system random");
    let s: String = b.iter().map(|x| ALPHABET[usize::from(x & 31)] as char).collect();
    format!("{}-{}", &s[..5], &s[5..])
}

/// The canonical form of what someone typed: upper case, no spaces or
/// dashes, the look-alikes O → 0 and I/L → 1. `None` when it can't be a code.
pub fn normalize(code: &str) -> Option<String> {
    let s: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect();
    (s.len() == CODE_LEN && s.bytes().all(|b| ALPHABET.contains(&b))).then_some(s)
}

fn finish(state: Spake2<Ed25519Group>, theirs: &[u8]) -> io::Result<Zeroizing<[u8; 32]>> {
    let k =
        Zeroizing::new(state.finish(theirs).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "pairing failed"))?);
    let mut out = Zeroizing::new([0u8; 32]);
    out.copy_from_slice(&k[..32]);
    Ok(out)
}

async fn spake_initiator<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    secret: &[u8],
    id: &[u8],
) -> io::Result<Zeroizing<[u8; 32]>> {
    let (state, msg) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(secret), &Identity::new(id));
    s.write_all(&msg).await?;
    s.flush().await?;
    let mut theirs = [0u8; SPAKE_MSG];
    s.read_exact(&mut theirs).await?;
    finish(state, &theirs)
}

async fn spake_responder<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    secret: &[u8],
    id: &[u8],
) -> io::Result<Zeroizing<[u8; 32]>> {
    let mut theirs = [0u8; SPAKE_MSG];
    s.read_exact(&mut theirs).await?;
    let (state, msg) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(secret), &Identity::new(id));
    s.write_all(&msg).await?;
    s.flush().await?;
    finish(state, &theirs)
}

/// SPAKE2 for the side that typed the code: sends first.
pub async fn initiator<S: AsyncRead + AsyncWrite + Unpin>(s: &mut S, code: &str) -> io::Result<Zeroizing<[u8; 32]>> {
    let code = Zeroizing::new(normalize(code).ok_or_else(|| io::Error::other("that isn't a pairing code"))?);
    spake_initiator(s, code.as_bytes(), ID).await
}

/// SPAKE2 for the side showing the code: answers.
pub async fn responder<S: AsyncRead + AsyncWrite + Unpin>(s: &mut S, code: &str) -> io::Result<Zeroizing<[u8; 32]>> {
    let code = Zeroizing::new(normalize(code).ok_or_else(|| io::Error::other("bad code"))?);
    spake_responder(s, code.as_bytes(), ID).await
}

/// SPAKE2 with the passphrase key (see [`super::discover::pass_key`]), for
/// the side that typed the passphrase.
pub async fn pass_initiator<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    key: &[u8; 32],
) -> io::Result<Zeroizing<[u8; 32]>> {
    spake_initiator(s, key, ID_PASS).await
}

/// SPAKE2 with the stored passphrase key, for the findable device.
pub async fn pass_responder<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    key: &[u8; 32],
) -> io::Result<Zeroizing<[u8; 32]>> {
    spake_responder(s, key, ID_PASS).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_look_right_and_normalize() {
        let c = new_code();
        assert_eq!(c.len(), CODE_LEN + 1);
        assert_eq!(&c[5..6], "-");
        assert_eq!(normalize(&c).unwrap(), c.replace('-', ""));
        assert_eq!(normalize("abcde fghjk").unwrap(), "ABCDEFGHJK");
        assert_eq!(normalize("0O1IL-23456").unwrap(), "0011123456");
        assert!(normalize("ABCDE-FGHJ").is_none());
        assert!(normalize("ABCDE-FGHJU").is_none());
        assert_ne!(new_code(), new_code());
    }

    #[tokio::test]
    async fn same_code_same_key() {
        let (mut x, mut y) = tokio::io::duplex(1024);
        let (a, b) = tokio::join!(initiator(&mut x, "abcde-12345"), responder(&mut y, "ABCDE12345"));
        assert_eq!(*a.unwrap(), *b.unwrap());

        let (mut x, mut y) = tokio::io::duplex(1024);
        let (a, b) = tokio::join!(initiator(&mut x, "abcde-12345"), responder(&mut y, "ABCDE12346"));
        assert_ne!(*a.unwrap(), *b.unwrap());
    }

    #[tokio::test]
    async fn passphrase_keys_agree_only_when_equal() {
        let (mut x, mut y) = tokio::io::duplex(1024);
        let k = [7u8; 32];
        let (a, b) = tokio::join!(pass_initiator(&mut x, &k), pass_responder(&mut y, &k));
        assert_eq!(*a.unwrap(), *b.unwrap());
        let (mut x, mut y) = tokio::io::duplex(1024);
        let (a, b) = tokio::join!(pass_initiator(&mut x, &k), pass_responder(&mut y, &[8u8; 32]));
        assert_ne!(*a.unwrap(), *b.unwrap());
    }
}

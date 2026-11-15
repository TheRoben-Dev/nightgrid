//! Minimal SOCKS5 server side (RFC 1928): no authentication, CONNECT only.
//! Enough for browsers and `curl --socks5-hostname`.

use std::net::{Ipv4Addr, Ipv6Addr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const VER: u8 = 5;
const NO_AUTH: u8 = 0;
const NO_ACCEPTABLE: u8 = 0xFF;
const CMD_CONNECT: u8 = 1;

/// Reply codes.
pub const REP_OK: u8 = 0;
pub const REP_FAILURE: u8 = 1;
pub const REP_HOST_UNREACHABLE: u8 = 4;
pub const REP_CMD_UNSUPPORTED: u8 = 7;
pub const REP_ATYP_UNSUPPORTED: u8 = 8;

/// Reads the greeting and CONNECT request. Returns the requested target, or
/// an error after replying to the client where the protocol allows it.
pub async fn accept<S: AsyncRead + AsyncWrite + Unpin>(s: &mut S) -> std::io::Result<(String, u16)> {
    let bad = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_owned());
    let mut head = [0u8; 2];
    s.read_exact(&mut head).await?;
    if head[0] != VER {
        return Err(bad("not a SOCKS5 client"));
    }
    let mut methods = vec![0u8; head[1] as usize];
    s.read_exact(&mut methods).await?;
    if !methods.contains(&NO_AUTH) {
        s.write_all(&[VER, NO_ACCEPTABLE]).await?;
        return Err(bad("client requires authentication"));
    }
    s.write_all(&[VER, NO_AUTH]).await?;

    let mut req = [0u8; 4];
    s.read_exact(&mut req).await?;
    if req[0] != VER {
        return Err(bad("bad request"));
    }
    let host = match req[3] {
        1 => {
            let mut a = [0u8; 4];
            s.read_exact(&mut a).await?;
            Ipv4Addr::from(a).to_string()
        }
        3 => {
            let mut len = [0u8; 1];
            s.read_exact(&mut len).await?;
            let mut name = vec![0u8; len[0] as usize];
            s.read_exact(&mut name).await?;
            String::from_utf8(name).map_err(|_| bad("bad host name"))?
        }
        4 => {
            let mut a = [0u8; 16];
            s.read_exact(&mut a).await?;
            Ipv6Addr::from(a).to_string()
        }
        _ => {
            reply(s, REP_ATYP_UNSUPPORTED).await?;
            return Err(bad("unsupported address type"));
        }
    };
    let mut port = [0u8; 2];
    s.read_exact(&mut port).await?;
    if req[1] != CMD_CONNECT {
        reply(s, REP_CMD_UNSUPPORTED).await?;
        return Err(bad("only CONNECT is supported"));
    }
    Ok((host, u16::from_be_bytes(port)))
}

/// Sends a reply with an unspecified bound address.
pub async fn reply<S: AsyncWrite + Unpin>(s: &mut S, code: u8) -> std::io::Result<()> {
    s.write_all(&[VER, code, 0, 1, 0, 0, 0, 0, 0, 0]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn run(client_bytes: Vec<u8>) -> (std::io::Result<(String, u16)>, Vec<u8>) {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move {
            b.write_all(&client_bytes).await.unwrap();
            let mut out = vec![0u8; 2];
            b.read_exact(&mut out).await.unwrap();
            out
        });
        let r = accept(&mut a).await;
        drop(a);
        (r, task.await.unwrap())
    }

    #[tokio::test]
    async fn domain_ipv4_ipv6() {
        let mut m = vec![5, 1, 0, 5, 1, 0, 3, 11];
        m.extend_from_slice(b"example.lan");
        m.extend_from_slice(&443u16.to_be_bytes());
        let (r, greet) = run(m).await;
        assert_eq!(greet, [5, 0]);
        assert_eq!(r.unwrap(), ("example.lan".into(), 443));

        let (r, _) = run(vec![5, 1, 0, 5, 1, 0, 1, 10, 0, 0, 7, 0, 80]).await;
        assert_eq!(r.unwrap(), ("10.0.0.7".into(), 80));

        let mut m = vec![5, 2, 2, 0, 5, 1, 0, 4];
        m.extend_from_slice(&"fd00::1".parse::<Ipv6Addr>().unwrap().octets());
        m.extend_from_slice(&[0x1f, 0x90]);
        let (r, _) = run(m).await;
        assert_eq!(r.unwrap(), ("fd00::1".into(), 8080));
    }

    #[tokio::test]
    async fn rejects_auth_only_and_socks4() {
        let (r, greet) = run(vec![5, 1, 2]).await;
        assert!(r.is_err());
        assert_eq!(greet, [5, 0xFF]);
        let (mut a, mut b) = tokio::io::duplex(64);
        b.write_all(&[4, 1, 0, 80, 1, 2, 3, 4, 0]).await.unwrap();
        assert!(accept(&mut a).await.is_err());
    }
}

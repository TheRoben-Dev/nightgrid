//! A small RFB (VNC) client, RFC 6143: versions 3.3 to 3.8, no-auth and
//! VNC-password security, Raw, CopyRect, ZRLE and DesktopSize.
//!
//! The stream is whatever carries the bytes: an SSH `direct-tcpip` channel to
//! a VNC port that only listens on the host's loopback, or `qm vncproxy` on an
//! exec channel. Nothing listens on this machine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use des::cipher::{BlockCipherEncrypt, KeyInit};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, Notify};
use zeroize::Zeroizing;

use super::frame::{Frame, Rect, MAX_SIDE};
use super::zrle::Zrle;

const ENC_RAW: i32 = 0;
const ENC_COPYRECT: i32 = 1;
const ENC_ZRLE: i32 = 16;
const ENC_DESKTOP_SIZE: i32 = -223;
/// Clipboard text from the guest larger than this is dropped.
const MAX_CUT: usize = 1 << 20;

/// What the UI sends to the guest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum VncInput {
    /// An X11 keysym going down or up.
    Key { keysym: u32, down: bool },
    /// Pointer position in framebuffer pixels; `buttons` is the RFB mask
    /// (1 left, 2 middle, 4 right, 8/16 wheel up/down).
    Pointer { x: u16, y: u16, buttons: u8 },
    /// Puts text on the guest's clipboard (needs a clipboard agent in the guest).
    Clipboard { text: String },
    /// Asks for the whole screen again.
    Refresh,
}

/// What the session reports besides pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum VncEvent {
    Connected {
        name: String,
        width: u16,
        height: u16,
    },
    Bell,
    /// Text the guest put on its clipboard.
    Clipboard {
        text: String,
    },
    /// The session ended (`reason` is empty when the user closed it).
    Closed {
        reason: String,
    },
}

/// State shared between the session task and the UI's frame requests.
#[derive(Default)]
pub struct Shared {
    pub frame: Mutex<Frame>,
    pub changed: Notify,
    pub closed: AtomicBool,
    /// Wakes the reader on close (a stored permit, so a close is never missed).
    stop: Notify,
}

impl Shared {
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        self.changed.notify_waiters();
        self.stop.notify_one();
    }
}

type Res<T> = Result<T, String>;

fn io(e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        "the display stream closed".into()
    } else {
        format!("display stream: {e}")
    }
}

/// The VNC-password response: DES over the challenge, keyed by the password
/// with each byte's bits reversed (a quirk of the original VNC).
pub fn vnc_auth_response(password: &str, challenge: &[u8; 16]) -> [u8; 16] {
    let mut key = Zeroizing::new([0u8; 8]);
    for (k, b) in key.iter_mut().zip(password.as_bytes()) {
        *k = b.reverse_bits();
    }
    let cipher = des::Des::new_from_slice(&key[..]).expect("8-byte key");
    let mut out = *challenge;
    for block in out.chunks_exact_mut(8) {
        cipher.encrypt_block(block.try_into().expect("8-byte block"));
    }
    out
}

async fn read_reason<R: AsyncRead + Unpin>(r: &mut R) -> String {
    let Ok(n) = r.read_u32().await else { return "refused".into() };
    let mut buf = vec![0; (n as usize).min(4096)];
    if r.read_exact(&mut buf).await.is_err() {
        return "refused".into();
    }
    String::from_utf8_lossy(&buf).trim().to_string()
}

/// Version and security handshake, then ClientInit/ServerInit. Returns the
/// desktop name and size.
pub async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    password: Option<&str>,
) -> Res<(String, u16, u16)> {
    let mut ver = [0u8; 12];
    s.read_exact(&mut ver).await.map_err(io)?;
    if &ver[..4] != b"RFB " || ver[11] != b'\n' {
        return Err("not a VNC server".into());
    }
    let text = String::from_utf8_lossy(&ver[4..11]).to_string();
    let (major, minor) = text
        .split_once('.')
        .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
        .ok_or("not a VNC server")?;
    let minor = if major > 3 || minor >= 8 {
        8
    } else if minor == 7 {
        7
    } else {
        3
    };
    s.write_all(format!("RFB 003.00{minor}\n").as_bytes()).await.map_err(io)?;

    let security = if minor == 3 {
        let t = s.read_u32().await.map_err(io)?;
        if t == 0 {
            return Err(format!("the server refused: {}", read_reason(s).await));
        }
        t
    } else {
        let n = s.read_u8().await.map_err(io)?;
        if n == 0 {
            return Err(format!("the server refused: {}", read_reason(s).await));
        }
        let mut types = vec![0u8; n.into()];
        s.read_exact(&mut types).await.map_err(io)?;
        let pick = if types.contains(&1) && password.is_none() {
            1
        } else if types.contains(&2) {
            2
        } else if types.contains(&1) {
            1
        } else {
            return Err(format!(
                "no supported VNC security type (server offers {types:?}; Nightgrid supports none and VNC password)"
            ));
        };
        s.write_u8(pick).await.map_err(io)?;
        u32::from(pick)
    };
    match security {
        1 => {}
        2 => {
            let pw = password.ok_or("this display needs a VNC password")?;
            let mut challenge = [0u8; 16];
            s.read_exact(&mut challenge).await.map_err(io)?;
            s.write_all(&vnc_auth_response(pw, &challenge)).await.map_err(io)?;
        }
        t => return Err(format!("unsupported VNC security type {t}")),
    }
    if security == 2 || minor == 8 {
        let ok = s.read_u32().await.map_err(io)?;
        if ok != 0 {
            let why = if minor == 8 { read_reason(s).await } else { String::new() };
            return Err(if why.is_empty() {
                "VNC authentication failed".into()
            } else {
                format!("VNC authentication failed: {why}")
            });
        }
    }

    s.write_u8(1).await.map_err(io)?; // shared: don't kick other viewers
    let w = s.read_u16().await.map_err(io)?;
    let h = s.read_u16().await.map_err(io)?;
    let mut fmt = [0u8; 16];
    s.read_exact(&mut fmt).await.map_err(io)?;
    let n = s.read_u32().await.map_err(io)? as usize;
    let mut name = vec![0; n.min(1024)];
    s.read_exact(&mut name).await.map_err(io)?;
    if n > 1024 {
        let mut rest = vec![0; n - 1024];
        s.read_exact(&mut rest).await.map_err(io)?;
    }
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("unsupported screen size {w}×{h}"));
    }
    Ok((String::from_utf8_lossy(&name).trim().to_string(), w, h))
}

/// SetPixelFormat (32-bit true colour, red in the low byte) and SetEncodings.
pub fn setup_messages() -> Vec<u8> {
    let mut m = vec![0, 0, 0, 0];
    m.extend_from_slice(&[32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 0, 8, 16, 0, 0, 0]);
    let encs = [ENC_ZRLE, ENC_COPYRECT, ENC_RAW, ENC_DESKTOP_SIZE];
    m.extend_from_slice(&[2, 0]);
    m.extend_from_slice(&(encs.len() as u16).to_be_bytes());
    for e in encs {
        m.extend_from_slice(&e.to_be_bytes());
    }
    m
}

pub fn update_request(incremental: bool, w: u16, h: u16) -> Vec<u8> {
    let mut m = vec![3, u8::from(incremental), 0, 0, 0, 0];
    m.extend_from_slice(&w.to_be_bytes());
    m.extend_from_slice(&h.to_be_bytes());
    m
}

/// The bytes for one input event (`size` clamps pointer positions).
pub fn encode_input(i: &VncInput, size: (u16, u16)) -> Vec<u8> {
    match i {
        VncInput::Key { keysym, down } => {
            let mut m = vec![4, u8::from(*down), 0, 0];
            m.extend_from_slice(&keysym.to_be_bytes());
            m
        }
        VncInput::Pointer { x, y, buttons } => {
            let mut m = vec![5, *buttons];
            m.extend_from_slice(&(*x).min(size.0.saturating_sub(1)).to_be_bytes());
            m.extend_from_slice(&(*y).min(size.1.saturating_sub(1)).to_be_bytes());
            m
        }
        VncInput::Clipboard { text } => {
            // RFB clipboard text is Latin-1.
            let bytes: Vec<u8> =
                text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).take(MAX_CUT).collect();
            let mut m = vec![6, 0, 0, 0];
            m.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            m.extend_from_slice(&bytes);
            m
        }
        VncInput::Refresh => update_request(false, size.0, size.1),
    }
}

/// Runs a session after the handshake until the stream ends or fails.
/// `input` closing (the UI closed the tab) ends it too.
pub async fn run<S>(
    stream: S,
    password: Option<Zeroizing<String>>,
    shared: Arc<Shared>,
    mut input: mpsc::Receiver<VncInput>,
    on_event: impl Fn(VncEvent) + Send + Sync + 'static,
) -> Res<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut stream = stream;
    let (name, w, h) = handshake(&mut stream, password.as_deref().map(|s| s.as_str())).await?;
    drop(password);
    *shared.frame.lock().expect("frame") = Frame::new(w, h);
    shared.changed.notify_waiters();
    on_event(VncEvent::Connected { name, width: w, height: h });

    let (mut rd, mut wr) = tokio::io::split(stream);
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut setup = setup_messages();
    setup.extend(update_request(false, w, h));
    let _ = out_tx.send(setup);
    let writer = tokio::spawn(async move {
        while let Some(m) = out_rx.recv().await {
            if wr.write_all(&m).await.is_err() {
                break;
            }
            let _ = wr.flush().await;
        }
    });
    let size = Arc::new(Mutex::new((w, h)));
    let input_task = {
        let out_tx = out_tx.clone();
        let size = size.clone();
        let shared = shared.clone();
        tokio::spawn(async move {
            while let Some(i) = input.recv().await {
                let sz = *size.lock().expect("size");
                if out_tx.send(encode_input(&i, sz)).is_err() {
                    break;
                }
            }
            // The UI went away: stop the reader too.
            shared.close();
        })
    };

    let mut zrle = Zrle::default();
    let result = loop {
        if shared.is_closed() {
            break Ok(());
        }
        let msg = tokio::select! {
            m = rd.read_u8() => m.map_err(io),
            _ = shared.stop.notified() => break Ok(()),
        };
        let r = match msg {
            Ok(0) => read_update(&mut rd, &shared, &mut zrle, &size).await.map(|_| {
                let (w, h) = *size.lock().expect("size");
                let _ = out_tx.send(update_request(true, w, h));
                shared.changed.notify_waiters();
            }),
            Ok(1) => skip_colour_map(&mut rd).await,
            Ok(2) => {
                on_event(VncEvent::Bell);
                Ok(())
            }
            Ok(3) => read_cut(&mut rd).await.map(|t| {
                if let Some(text) = t {
                    on_event(VncEvent::Clipboard { text });
                }
            }),
            Ok(t) => Err(format!("unknown VNC message {t}")),
            Err(e) => Err(e),
        };
        if let Err(e) = r {
            break if shared.is_closed() { Ok(()) } else { Err(e) };
        }
    };
    writer.abort();
    input_task.abort();
    shared.close();
    result
}

async fn read_update<R: AsyncRead + Unpin>(
    r: &mut R,
    shared: &Shared,
    zrle: &mut Zrle,
    size: &Mutex<(u16, u16)>,
) -> Res<()> {
    r.read_u8().await.map_err(io)?;
    let n = r.read_u16().await.map_err(io)?;
    for _ in 0..n {
        let rect = Rect {
            x: r.read_u16().await.map_err(io)?,
            y: r.read_u16().await.map_err(io)?,
            w: r.read_u16().await.map_err(io)?,
            h: r.read_u16().await.map_err(io)?,
        };
        let enc = r.read_i32().await.map_err(io)?;
        if enc == ENC_DESKTOP_SIZE {
            if rect.w == 0 || rect.h == 0 || rect.w > MAX_SIDE || rect.h > MAX_SIDE {
                return Err(format!("unsupported screen size {}×{}", rect.w, rect.h));
            }
            shared.frame.lock().expect("frame").resize(rect.w, rect.h);
            *size.lock().expect("size") = (rect.w, rect.h);
            continue;
        }
        if !shared.frame.lock().expect("frame").fits(rect) {
            return Err("the server sent a rectangle outside the screen".into());
        }
        match enc {
            ENC_RAW => {
                let mut buf = vec![0u8; usize::from(rect.w) * usize::from(rect.h) * 4];
                r.read_exact(&mut buf).await.map_err(io)?;
                let mut f = shared.frame.lock().expect("frame");
                f.blit_rgbx(rect, &buf);
                f.mark(rect);
            }
            ENC_COPYRECT => {
                let sx = r.read_u16().await.map_err(io)?;
                let sy = r.read_u16().await.map_err(io)?;
                let mut f = shared.frame.lock().expect("frame");
                if !f.fits(Rect { x: sx, y: sy, w: rect.w, h: rect.h }) {
                    return Err("the server copied from outside the screen".into());
                }
                f.copy(rect, sx, sy);
                f.mark(rect);
            }
            ENC_ZRLE => {
                let len = r.read_u32().await.map_err(io)? as usize;
                if len > 64 << 20 {
                    return Err("ZRLE rectangle too large".into());
                }
                let mut buf = vec![0u8; len];
                r.read_exact(&mut buf).await.map_err(io)?;
                let mut f = shared.frame.lock().expect("frame");
                zrle.decode(&buf, rect, &mut f)?;
                f.mark(rect);
            }
            e => return Err(format!("the server used an encoding Nightgrid didn't ask for ({e})")),
        }
    }
    Ok(())
}

async fn skip_colour_map<R: AsyncRead + Unpin>(r: &mut R) -> Res<()> {
    let mut head = [0u8; 5];
    r.read_exact(&mut head).await.map_err(io)?;
    let n = u16::from_be_bytes([head[3], head[4]]) as usize;
    let mut buf = vec![0u8; n * 6];
    r.read_exact(&mut buf).await.map_err(io).map(|_| ())
}

async fn read_cut<R: AsyncRead + Unpin>(r: &mut R) -> Res<Option<String>> {
    let mut pad = [0u8; 3];
    r.read_exact(&mut pad).await.map_err(io)?;
    let n = r.read_u32().await.map_err(io)? as usize;
    if n > MAX_CUT {
        let mut left = n;
        let mut buf = vec![0u8; 64 * 1024];
        while left > 0 {
            let k = left.min(buf.len());
            r.read_exact(&mut buf[..k]).await.map_err(io)?;
            left -= k;
        }
        return Ok(None);
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).await.map_err(io)?;
    Ok(Some(buf.iter().map(|&b| b as char).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vnc::zrle::tests::pack;
    use flate2::{Compress, Compression};
    use tokio::io::duplex;

    #[test]
    fn vnc_auth_matches_the_reference() {
        // Reference from `openssl enc -des-ecb` keyed with "password" bit-reversed
        // (0e86ceceeef64e26), challenge bytes 0..16.
        let challenge: [u8; 16] = core::array::from_fn(|i| i as u8);
        let r = vnc_auth_response("password", &challenge);
        let want = [0xb8, 0x66, 0x92, 0x41, 0x25, 0xc8, 0xee, 0xbb, 0x9d, 0xeb, 0xc1, 0xdb, 0x61, 0xc5, 0x38, 0xe2];
        assert_eq!(r, want);
        // Only the first 8 characters count.
        assert_eq!(r, vnc_auth_response("password-and-more", &challenge));
        assert_ne!(r, vnc_auth_response("passwore", &challenge));
    }

    #[test]
    fn input_encoding() {
        assert_eq!(
            encode_input(&VncInput::Key { keysym: 0xff0d, down: true }, (10, 10)),
            vec![4, 1, 0, 0, 0, 0, 0xff, 0x0d]
        );
        assert_eq!(encode_input(&VncInput::Pointer { x: 50, y: 3, buttons: 1 }, (10, 10)), vec![5, 1, 0, 9, 0, 3]);
        assert_eq!(
            encode_input(&VncInput::Clipboard { text: "hé€".into() }, (1, 1)),
            vec![6, 0, 0, 0, 0, 0, 0, 3, b'h', 0xe9, b'?']
        );
        assert_eq!(encode_input(&VncInput::Refresh, (800, 600)), update_request(false, 800, 600));
    }

    async fn server_init(s: &mut tokio::io::DuplexStream, w: u16, h: u16) {
        assert_eq!(s.read_u8().await.unwrap(), 1, "shared flag");
        s.write_u16(w).await.unwrap();
        s.write_u16(h).await.unwrap();
        s.write_all(&[0u8; 16]).await.unwrap();
        s.write_u32(4).await.unwrap();
        s.write_all(b"web1").await.unwrap();
    }

    #[tokio::test]
    async fn handshake_38_with_password() {
        let (mut c, mut s) = duplex(4096);
        let server = tokio::spawn(async move {
            s.write_all(b"RFB 003.008\n").await.unwrap();
            let mut v = [0u8; 12];
            s.read_exact(&mut v).await.unwrap();
            assert_eq!(&v, b"RFB 003.008\n");
            s.write_all(&[2, 1, 2]).await.unwrap();
            assert_eq!(s.read_u8().await.unwrap(), 2);
            let ch = [7u8; 16];
            s.write_all(&ch).await.unwrap();
            let mut resp = [0u8; 16];
            s.read_exact(&mut resp).await.unwrap();
            assert_eq!(resp, vnc_auth_response("secret", &ch));
            s.write_u32(0).await.unwrap();
            server_init(&mut s, 640, 480).await;
        });
        let (name, w, h) = handshake(&mut c, Some("secret")).await.unwrap();
        assert_eq!((name.as_str(), w, h), ("web1", 640, 480));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn handshake_33_and_failures() {
        let (mut c, mut s) = duplex(4096);
        tokio::spawn(async move {
            s.write_all(b"RFB 003.003\n").await.unwrap();
            let mut v = [0u8; 12];
            s.read_exact(&mut v).await.unwrap();
            assert_eq!(&v, b"RFB 003.003\n");
            s.write_u32(1).await.unwrap();
            server_init(&mut s, 800, 600).await;
        });
        assert_eq!(handshake(&mut c, None).await.unwrap().1, 800);

        // Wrong password under 3.8 carries a reason.
        let (mut c, mut s) = duplex(4096);
        tokio::spawn(async move {
            s.write_all(b"RFB 003.008\n").await.unwrap();
            let mut v = [0u8; 12];
            s.read_exact(&mut v).await.unwrap();
            s.write_all(&[1, 2]).await.unwrap();
            s.read_u8().await.unwrap();
            s.write_all(&[0u8; 16]).await.unwrap();
            let mut resp = [0u8; 16];
            s.read_exact(&mut resp).await.unwrap();
            s.write_u32(1).await.unwrap();
            s.write_u32(9).await.unwrap();
            s.write_all(b"bad login").await.unwrap();
        });
        let e = handshake(&mut c, Some("nope")).await.unwrap_err();
        assert!(e.contains("bad login"), "{e}");

        // A password-only server without a password.
        let (mut c, mut s) = duplex(4096);
        tokio::spawn(async move {
            s.write_all(b"RFB 003.008\n").await.unwrap();
            let mut v = [0u8; 12];
            s.read_exact(&mut v).await.unwrap();
            s.write_all(&[1, 2]).await.unwrap();
            s.read_u8().await.unwrap();
        });
        assert!(handshake(&mut c, None).await.unwrap_err().contains("password"));

        let (mut c, mut s) = duplex(4096);
        tokio::spawn(async move { s.write_all(b"SSH-2.0-xyz\n").await.unwrap() });
        assert!(handshake(&mut c, None).await.unwrap_err().contains("not a VNC server"));
    }

    #[tokio::test]
    async fn session_draws_updates_and_relays_input() {
        let (c, mut s) = duplex(1 << 16);
        let shared = Arc::new(Shared::default());
        let (in_tx, in_rx) = mpsc::channel(8);
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let session = tokio::spawn(run(c, None, shared.clone(), in_rx, move |e| ev.lock().unwrap().push(e)));

        s.write_all(b"RFB 003.008\n").await.unwrap();
        let mut v = [0u8; 12];
        s.read_exact(&mut v).await.unwrap();
        s.write_all(&[1, 1]).await.unwrap();
        s.read_u8().await.unwrap();
        s.write_u32(0).await.unwrap();
        server_init(&mut s, 4, 2).await;

        // SetPixelFormat + SetEncodings + full update request.
        let mut setup = vec![0u8; setup_messages().len() + 10];
        s.read_exact(&mut setup).await.unwrap();
        assert_eq!(setup[0], 0);
        assert_eq!(&setup[setup.len() - 10..], &update_request(false, 4, 2)[..]);

        // One update: a raw pixel, a ZRLE solid tile, a copy.
        let mut m = vec![0, 0, 0, 3];
        m.extend_from_slice(&[0, 0, 0, 0, 0, 1, 0, 1]);
        m.extend_from_slice(&ENC_RAW.to_be_bytes());
        m.extend_from_slice(&[10, 20, 30, 0]);
        let z = pack(&mut Compress::new(Compression::default(), true), &[1, 40, 50, 60]);
        m.extend_from_slice(&[0, 2, 0, 0, 0, 2, 0, 2]);
        m.extend_from_slice(&ENC_ZRLE.to_be_bytes());
        m.extend_from_slice(&(z.len() as u32).to_be_bytes());
        m.extend_from_slice(&z);
        m.extend_from_slice(&[0, 0, 0, 1, 0, 1, 0, 1]);
        m.extend_from_slice(&ENC_COPYRECT.to_be_bytes());
        m.extend_from_slice(&[0, 0, 0, 0]);
        m.push(2); // bell
        s.write_all(&m).await.unwrap();

        // The client asks for the next incremental update.
        let mut req = [0u8; 10];
        s.read_exact(&mut req).await.unwrap();
        assert_eq!(req, update_request(true, 4, 2)[..]);
        {
            let f = shared.frame.lock().unwrap();
            assert_eq!(&f.pixels[0..4], &[10, 20, 30, 255]);
            assert_eq!(&f.pixels[4 * 4..4 * 4 + 3], &[10, 20, 30], "copied to (0,1)");
            assert_eq!(&f.pixels[(4 + 3) * 4..(4 + 3) * 4 + 3], &[40, 50, 60]);
        }

        in_tx.send(VncInput::Key { keysym: 0x61, down: true }).await.unwrap();
        let mut key = [0u8; 8];
        s.read_exact(&mut key).await.unwrap();
        assert_eq!(key, [4, 1, 0, 0, 0, 0, 0, 0x61]);

        // Resize, then the server hangs up.
        let mut m = vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 8, 0, 6];
        m.extend_from_slice(&ENC_DESKTOP_SIZE.to_be_bytes());
        s.write_all(&m).await.unwrap();
        let mut req = [0u8; 10];
        s.read_exact(&mut req).await.unwrap();
        assert_eq!(req, update_request(true, 8, 6)[..]);
        drop(s);
        let r = session.await.unwrap();
        assert!(r.unwrap_err().contains("closed"));
        assert_eq!(shared.frame.lock().unwrap().width, 8);
        let ev = events.lock().unwrap();
        assert!(matches!(&ev[0], VncEvent::Connected { name, width: 4, height: 2 } if name == "web1"));
        assert!(ev.contains(&VncEvent::Bell));
    }

    #[tokio::test]
    async fn closing_input_ends_the_session() {
        let (c, mut s) = duplex(1 << 16);
        let shared = Arc::new(Shared::default());
        let (in_tx, in_rx) = mpsc::channel(8);
        let session = tokio::spawn(run(c, None, shared.clone(), in_rx, |_| {}));
        s.write_all(b"RFB 003.008\n").await.unwrap();
        let mut v = [0u8; 12];
        s.read_exact(&mut v).await.unwrap();
        s.write_all(&[1, 1]).await.unwrap();
        s.read_u8().await.unwrap();
        s.write_u32(0).await.unwrap();
        server_init(&mut s, 4, 2).await;
        drop(in_tx);
        assert!(tokio::time::timeout(std::time::Duration::from_secs(5), session).await.unwrap().unwrap().is_ok());
    }
}

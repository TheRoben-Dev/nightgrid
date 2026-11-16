//! VM desktops (M10n): a built-in VNC viewer whose bytes travel over the
//! host's SSH connection, and RDP through an SSH tunnel.

pub mod frame;
pub mod rfb;
pub mod target;
pub mod zrle;

use zeroize::Zeroizing;

use crate::ssh::client::Connection;
use crate::system::sudo::{self, SudoAuth};
use crate::system::vms::{valid_id, VmKind};
use crate::system::{SystemError, SystemResult};
use target::Display;

/// The byte stream a session reads and writes (an SSH channel).
pub type Stream = russh::ChannelStream<russh::client::Msg>;

/// A guest's display, reached and ready for the RFB handshake.
pub struct Opened {
    pub stream: Stream,
    /// The VNC password, if the display has one (never stored).
    pub password: Option<Zeroizing<String>>,
}

/// Finds a guest's display on the host and opens a channel to it.
pub async fn open(conn: &Connection, auth: &SudoAuth, kind: VmKind, id: &str) -> SystemResult<Opened> {
    if !valid_id(kind, id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")));
    }
    match kind {
        VmKind::Libvirt => {
            let out = sudo::run(conn, auth, &target::libvirt_display_command(id)?).await?;
            match target::parse_libvirt_display(&out.stdout).map_err(SystemError::other)? {
                Display::Tcp { host, port, password } => {
                    let ch = conn
                        .handle
                        .channel_open_direct_tcpip(host.clone(), port.into(), "127.0.0.1", 0)
                        .await
                        .map_err(|e| {
                            SystemError::other(format!(
                                "couldn't reach the VM's display at {host}:{port} through SSH: {e}"
                            ))
                        })?;
                    Ok(Opened { stream: ch.into_stream(), password: password.map(Zeroizing::new) })
                }
                Display::Unix { path, password } => {
                    let ch = conn.open_streamlocal(&path).await.map_err(|e| {
                        SystemError::other(format!(
                            "couldn't open the display socket {path} through SSH ({e}); give the VM a VNC listener on 127.0.0.1 instead"
                        ))
                    })?;
                    Ok(Opened { stream: ch.into_stream(), password: password.map(Zeroizing::new) })
                }
            }
        }
        VmKind::Qemu => {
            let out = sudo::run(conn, auth, &target::qm_status_command(id)?).await?;
            target::parse_qm_status(&out.stdout).map_err(SystemError::other)?;
            let pw = Zeroizing::new(target::session_password());
            let ch = sudo::open_interactive(conn, auth, &target::qm_vncproxy_command(id)?).await?;
            let mut line = Zeroizing::new(pw.as_bytes().to_vec());
            line.push(b'\n');
            ch.data(&line[..]).await.map_err(SystemError::other)?;
            Ok(Opened { stream: ch.into_stream(), password: Some(pw) })
        }
        VmKind::Lxc => Err(SystemError::other("containers have no screen: open a console instead")),
    }
}

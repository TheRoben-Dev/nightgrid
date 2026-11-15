//! Power: reboot, shut down and suspend a host, wake it with Wake-on-LAN, and
//! tell a planned power state apart from an outage (M10k).
//!
//! A host that stops answering is "offline" unless Nightgrid knows why:
//! - it ran a power action on it ([`note`] from the app);
//! - systemd-logind announced sleep or shutdown over an open SSH session
//!   ([`WATCH_COMMAND`] / [`parse_watch_line`]);
//! - the host is inside its expected sleep window ([`SleepWindow`]).
//!
//! Those hosts show "Suspended", "Rebooting", "Shutting down" or "Asleep"
//! instead, and raise no offline alert. A note clears when the host answers
//! again; "Rebooting" also gives up after [`REBOOT_GRACE`] (then it really is
//! offline).

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{LazyLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::{Error, Result};

/// A power action the user starts from Nightgrid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PowerAction {
    Reboot,
    Shutdown,
    Suspend,
}

impl PowerAction {
    /// Run as root (through sudo). Falls back from systemd to the classic tools.
    pub fn command(self) -> &'static str {
        match self {
            Self::Reboot => "systemctl reboot 2>&1 || shutdown -r now 2>&1",
            Self::Shutdown => "systemctl poweroff 2>&1 || shutdown -h now 2>&1",
            Self::Suspend => "systemctl suspend 2>&1 || loginctl suspend 2>&1 || pm-suspend 2>&1",
        }
    }

    pub fn state(self) -> PowerState {
        match self {
            Self::Reboot => PowerState::Rebooting,
            Self::Shutdown => PowerState::ShuttingDown,
            Self::Suspend => PowerState::Suspended,
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            Self::Reboot => "reboot",
            Self::Shutdown => "shut down",
            Self::Suspend => "suspend",
        }
    }
}

/// Why a host that doesn't answer isn't an outage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PowerState {
    Suspended,
    Rebooting,
    ShuttingDown,
    /// Inside the host's expected sleep window.
    Asleep,
}

impl PowerState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Suspended => "Suspended",
            Self::Rebooting => "Rebooting",
            Self::ShuttingDown => "Shutting down",
            Self::Asleep => "Asleep",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PowerNote {
    pub state: PowerState,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub since: i64,
    /// In words: "suspended from Nightgrid", "the host announced sleep".
    pub reason: String,
}

/// After this long a "Rebooting" host that still doesn't answer is offline.
pub const REBOOT_GRACE: i64 = 15 * 60;

static NOTES: LazyLock<RwLock<HashMap<HostId, PowerNote>>> = LazyLock::new(Default::default);

/// Records a planned power state for `host`.
pub fn note(host: HostId, state: PowerState, reason: impl Into<String>, now: i64) {
    NOTES.write().expect("poisoned").insert(host, PowerNote { state, since: now, reason: reason.into() });
}

/// The host answered: whatever it was doing, it's back.
pub fn clear(host: HostId) {
    NOTES.write().expect("poisoned").remove(&host);
}

/// The planned state of a host that doesn't answer, if any.
pub fn current(host: HostId, window: Option<&SleepWindow>, local_minute: u16, now: i64) -> Option<PowerNote> {
    let n = NOTES.read().expect("poisoned").get(&host).cloned();
    match n {
        Some(n) if n.state == PowerState::Rebooting && now - n.since > REBOOT_GRACE => {
            clear(host);
        }
        Some(n) => return Some(n),
        None => {}
    }
    window.filter(|w| w.contains(local_minute)).map(|w| PowerNote {
        state: PowerState::Asleep,
        since: now,
        reason: format!("inside its sleep window ({w})"),
    })
}

// ---- expected sleep window ---------------------------------------------

/// Local times the host is expected to be asleep, e.g. `23:00-07:30`
/// (minutes after midnight; may wrap past midnight).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SleepWindow {
    pub start: u16,
    pub end: u16,
}

fn hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl SleepWindow {
    pub fn parse(s: &str) -> Result<Self> {
        let bad = || Error::Invalid { field: "sleepWindow", reason: "use HH:MM-HH:MM, e.g. 23:00-07:00".into() };
        let (a, b) = s.split_once('-').ok_or_else(bad)?;
        let (start, end) = (hhmm(a).ok_or_else(bad)?, hhmm(b).ok_or_else(bad)?);
        if start == end {
            return Err(bad());
        }
        Ok(Self { start, end })
    }

    pub fn contains(&self, minute: u16) -> bool {
        if self.start < self.end {
            (self.start..self.end).contains(&minute)
        } else {
            minute >= self.start || minute < self.end
        }
    }
}

impl std::fmt::Display for SleepWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}-{:02}:{:02}", self.start / 60, self.start % 60, self.end / 60, self.end % 60)
    }
}

// ---- logind signals ------------------------------------------------------

/// Follows systemd-logind's PrepareForSleep / PrepareForShutdown signals.
/// Runs unprivileged (signals are broadcast); prints nothing useful where
/// neither `gdbus` nor `dbus-monitor` exists, and exits at once without D-Bus.
pub const WATCH_COMMAND: &str = "if command -v gdbus >/dev/null 2>&1; then \
     exec gdbus monitor --system --dest org.freedesktop.login1 --object-path /org/freedesktop/login1; \
     elif command -v dbus-monitor >/dev/null 2>&1; then \
     exec dbus-monitor --system \"type='signal',interface='org.freedesktop.login1.Manager'\"; \
     else echo 'no-dbus-tools'; fi";

/// What a line of [`WATCH_COMMAND`] output announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watch {
    Sleep,
    Shutdown,
    /// Woke up (PrepareForSleep false).
    Resumed,
}

/// Parses gdbus (`...Manager.PrepareForSleep (true,)`) and dbus-monitor
/// (`member=PrepareForSleep` then `boolean true`) output. dbus-monitor puts
/// the value on the next line, so the caller keeps the member it last saw.
pub fn parse_watch_line(line: &str, pending: &mut Option<&'static str>) -> Option<Watch> {
    let member = if line.contains("PrepareForSleep") {
        Some("sleep")
    } else if line.contains("PrepareForShutdown") {
        Some("shutdown")
    } else {
        None
    };
    let value = if line.contains("(true") || line.trim() == "boolean true" {
        Some(true)
    } else if line.contains("(false") || line.trim() == "boolean false" {
        Some(false)
    } else {
        None
    };
    let member = match (member, value) {
        (Some(m), Some(_)) => m,
        (Some(m), None) => {
            *pending = Some(m);
            return None;
        }
        (None, Some(_)) => pending.take()?,
        (None, None) => return None,
    };
    match (member, value?) {
        ("sleep", true) => Some(Watch::Sleep),
        ("sleep", false) => Some(Watch::Resumed),
        ("shutdown", true) => Some(Watch::Shutdown),
        _ => None,
    }
}

/// Runs [`WATCH_COMMAND`] on `conn` until the channel closes, calling `on`
/// for each announcement.
pub async fn watch(conn: &crate::ssh::client::Connection, mut on: impl FnMut(Watch)) {
    let Ok(mut ch) = conn.open_exec(WATCH_COMMAND).await else { return };
    let mut buf = String::new();
    let mut pending = None;
    while let Some(msg) = ch.wait().await {
        match msg {
            russh::ChannelMsg::Data { data } => buf.push_str(&String::from_utf8_lossy(&data)),
            russh::ChannelMsg::Eof | russh::ChannelMsg::Close => break,
            _ => continue,
        }
        while let Some(i) = buf.find('\n') {
            let line: String = buf.drain(..=i).collect();
            if let Some(w) = parse_watch_line(&line, &mut pending) {
                on(w);
            }
        }
        if buf.len() > 64 * 1024 {
            buf.clear();
        }
    }
}

// ---- Wake-on-LAN ---------------------------------------------------------

/// `aa:bb:cc:dd:ee:ff`, `aa-bb-…` or `aabbccddeeff`.
pub fn parse_mac(s: &str) -> Result<[u8; 6]> {
    let hex: String = s.chars().filter(|c| !matches!(c, ':' | '-' | '.' | ' ')).collect();
    let bad = || Error::Invalid { field: "macAddress", reason: "six hex bytes, e.g. aa:bb:cc:dd:ee:ff".into() };
    if hex.len() != 12 {
        return Err(bad());
    }
    let mut mac = [0u8; 6];
    for (i, b) in mac.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| bad())?;
    }
    Ok(mac)
}

pub fn format_mac(mac: &[u8; 6]) -> String {
    mac.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")
}

/// Six 0xff bytes, then the MAC sixteen times.
pub fn magic_packet(mac: &[u8; 6]) -> [u8; 102] {
    let mut p = [0xffu8; 102];
    for i in 0..16 {
        p[6 + i * 6..12 + i * 6].copy_from_slice(mac);
    }
    p
}

/// Sends the magic packet from this computer: to the limited broadcast
/// address and, when given, to a directed broadcast (`192.168.1.255`), on
/// UDP ports 9 and 7.
pub async fn wake_local(mac: &[u8; 6], broadcast: Option<Ipv4Addr>) -> std::io::Result<()> {
    let sock = tokio::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    sock.set_broadcast(true)?;
    let pkt = magic_packet(mac);
    let mut targets = vec![Ipv4Addr::BROADCAST];
    targets.extend(broadcast.filter(|b| *b != Ipv4Addr::BROADCAST));
    let mut last = Ok(());
    let mut sent = false;
    for ip in targets {
        for port in [9u16, 7] {
            match sock.send_to(&pkt, SocketAddr::from((ip, port))).await {
                Ok(_) => sent = true,
                Err(e) => last = Err(e),
            }
        }
    }
    if sent {
        Ok(())
    } else {
        last
    }
}

/// Sends the magic packet from another host on the sleeping host's LAN
/// (useful away from home): `wakeonlan`, `etherwake`, or Python.
pub fn wake_relay_command(mac: &[u8; 6]) -> String {
    let m = format_mac(mac);
    let hex: String = mac.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "if command -v wakeonlan >/dev/null 2>&1; then wakeonlan {m}; \
         elif command -v python3 >/dev/null 2>&1; then python3 -c \"import socket;s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);\
s.setsockopt(socket.SOL_SOCKET,socket.SO_BROADCAST,1);p=bytes.fromhex('ff'*6+'{hex}'*16);[s.sendto(p,('255.255.255.255',port)) for port in (9,7)];print('sent')\"; \
         else echo 'needs wakeonlan or python3 on the relay host' >&2; exit 3; fi"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macs_and_packets() {
        let m = parse_mac("AA-bb-cc-01-02-03").unwrap();
        assert_eq!(format_mac(&m), "aa:bb:cc:01:02:03");
        assert_eq!(parse_mac("aabbcc010203").unwrap(), m);
        assert!(parse_mac("aa:bb:cc").is_err());
        assert!(parse_mac("zz:bb:cc:01:02:03").is_err());
        let p = magic_packet(&m);
        assert_eq!(&p[..6], &[0xff; 6]);
        assert_eq!(&p[6..12], &m);
        assert_eq!(&p[96..], &m);
        assert!(wake_relay_command(&m).contains("aabbcc010203"));
    }

    #[test]
    fn windows() {
        let w = SleepWindow::parse("23:00-07:30").unwrap();
        assert!(w.contains(23 * 60) && w.contains(3 * 60) && !w.contains(7 * 60 + 30) && !w.contains(12 * 60));
        assert_eq!(w.to_string(), "23:00-07:30");
        let d = SleepWindow::parse("01:00 - 05:00").unwrap();
        assert!(d.contains(60) && !d.contains(5 * 60) && !d.contains(0));
        assert!(SleepWindow::parse("25:00-01:00").is_err());
        assert!(SleepWindow::parse("01:00-01:00").is_err());
        assert!(SleepWindow::parse("nope").is_err());
    }

    #[test]
    fn notes_expire_and_windows_apply() {
        let h: HostId = 9_001;
        let w = SleepWindow::parse("22:00-06:00").unwrap();
        assert_eq!(current(h, None, 0, 1000), None);
        assert_eq!(current(h, Some(&w), 23 * 60, 1000).unwrap().state, PowerState::Asleep);
        note(h, PowerState::Rebooting, "rebooted from Nightgrid", 1000);
        assert_eq!(current(h, None, 0, 1100).unwrap().state, PowerState::Rebooting);
        assert_eq!(current(h, None, 0, 1000 + REBOOT_GRACE + 1), None);
        note(h, PowerState::Suspended, "x", 1000);
        assert_eq!(current(h, None, 0, 1_000_000).unwrap().state, PowerState::Suspended);
        clear(h);
        assert_eq!(current(h, None, 12 * 60, 1000), None);
    }

    #[test]
    fn logind_lines() {
        let mut p = None;
        let g = "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (true,)";
        assert_eq!(parse_watch_line(g, &mut p), Some(Watch::Sleep));
        let g = "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (false,)";
        assert_eq!(parse_watch_line(g, &mut p), Some(Watch::Resumed));
        let g = "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForShutdown (true,)";
        assert_eq!(parse_watch_line(g, &mut p), Some(Watch::Shutdown));
        // dbus-monitor
        let d = "signal time=1700000000.1 sender=:1.2 -> destination=(null destination) serial=9 path=/org/freedesktop/login1; interface=org.freedesktop.login1.Manager; member=PrepareForSleep";
        assert_eq!(parse_watch_line(d, &mut p), None);
        assert_eq!(parse_watch_line("   boolean true", &mut p), Some(Watch::Sleep));
        assert_eq!(parse_watch_line("   boolean true", &mut p), None);
        assert_eq!(parse_watch_line("something else", &mut p), None);
    }
}

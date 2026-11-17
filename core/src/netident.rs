//! Device identity for the network map: names from reverse DNS, DHCP
//! leases, NetBIOS and UPnP, and a guess at what each device is.
//!
//! All of it comes from the scanning host (rule 7): its resolver, its own
//! lease files when it is the DHCP server, `nmblookup` when Samba's tools are
//! there, and a UPnP search on its LAN with `python3`. The guess only uses
//! what the scan already saw; the user's label always wins in the UI.

use std::net::IpAddr;

use crate::netmap::{Device, Seen};

/// `192.168.1.20 printer.lan` lines (reverse DNS, NetBIOS).
pub fn parse_names(lines: &[&str]) -> Vec<(String, String)> {
    lines
        .iter()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let ip = w.next()?;
            ip.parse::<IpAddr>().ok()?;
            let name = clean_name(w.next()?)?;
            Some((ip.to_owned(), name))
        })
        .collect()
}

/// A name worth showing: trailing dot dropped, no bare addresses, no
/// placeholders.
pub fn clean_name(n: &str) -> Option<String> {
    let n = n.trim().trim_end_matches('.');
    if n.is_empty() || n == "*" || n.parse::<IpAddr>().is_ok() || n.len() > 120 || n.contains(char::is_control) {
        return None;
    }
    // ISP-style reverse names for an address ("192-168-1-20.lan") say nothing.
    let first = n.split('.').next().unwrap_or(n);
    if first.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return None;
    }
    Some(n.to_owned())
}

/// DHCP leases from the scanning host when it hands out addresses:
/// `dnsmasq <expiry> <mac> <ip> <name> <client-id>` (dnsmasq, Pi-hole) and
/// `isc <ip> <mac> <name>` (ISC dhcpd, pre-digested by the script).
pub fn parse_leases(lines: &[&str]) -> Vec<(String, Option<String>, String)> {
    lines
        .iter()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let (ip, mac, name) = match *w.first()? {
                "dnsmasq" if w.len() >= 5 => (w[3], w[2], w[4]),
                "isc" if w.len() >= 4 => (w[1], w[2], w[3]),
                _ => return None,
            };
            ip.parse::<IpAddr>().ok()?;
            let mac = (mac.len() == 17 && mac.split(':').count() == 6).then(|| mac.to_ascii_lowercase());
            Some((ip.to_owned(), mac, clean_name(name)?))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsdpDevice {
    pub ip: String,
    /// `SERVER:` header ("Linux/5.4 UPnP/1.0 Sonos/72.1").
    pub server: String,
    pub friendly: String,
    pub maker: String,
    pub model: String,
    /// `urn:schemas-upnp-org:device:MediaRenderer:1` → `MediaRenderer`.
    pub device_type: String,
}

/// Tab-separated `ip server friendlyName manufacturer modelName deviceType`.
pub fn parse_ssdp(lines: &[&str]) -> Vec<SsdpDevice> {
    lines
        .iter()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').map(str::trim).collect();
            let ip = *f.first()?;
            ip.parse::<IpAddr>().ok()?;
            let get = |i: usize| f.get(i).map(|s| s.chars().take(120).collect::<String>()).unwrap_or_default();
            let dt = get(5);
            let device_type = dt.split(':').rev().nth(1).unwrap_or("").to_owned();
            Some(SsdpDevice {
                ip: ip.to_owned(),
                server: get(1),
                friendly: get(2),
                maker: get(3),
                model: get(4),
                device_type,
            })
        })
        .collect()
}

fn any(hay: &str, words: &[&str]) -> bool {
    words.iter().any(|w| hay.contains(w))
}

/// What a device probably is, as a `device_labels::KINDS` id.
pub fn guess_kind(d: &Device, gateway: Option<&str>) -> &'static str {
    if gateway == Some(d.ip.as_str()) || d.seen.contains(&Seen::Gateway) {
        return "router";
    }
    let svc = d.services.join(" ").to_ascii_lowercase();
    let names = d.names.join(" ").to_ascii_lowercase();
    let vendor = d.vendor.as_deref().unwrap_or("").to_ascii_lowercase();
    let model = d.model.as_deref().unwrap_or("").to_ascii_lowercase();
    let os = d.os.as_deref().unwrap_or("").to_ascii_lowercase();
    let all = format!("{names} {model}");

    // UPnP device types say it outright.
    if any(&svc, &["upnp:internetgatewaydevice", "upnp:wfadevice"]) {
        return "router";
    }
    if any(&svc, &["upnp:printer", "_ipp.", "_ipps.", "_printer.", "_pdl-datastream.", "_scanner."]) {
        return "printer";
    }
    // Names people and makers give devices.
    if any(&all, &["ipad", "galaxy tab", "tablet"]) {
        return "tablet";
    }
    if any(&all, &["iphone", "android", "pixel", "galaxy", "oneplus", "phone"]) {
        return "phone";
    }
    if any(&all, &["xbox", "playstation", "ps5", "ps4", "nintendo", "switch-", "steamdeck", "steam-deck"]) {
        return "console";
    }
    if any(&all, &["macbook", "laptop", "thinkpad", "zenbook", "xps"]) {
        return "laptop";
    }
    if any(&all, &["printer", "laserjet", "officejet", "deskjet", "envy", "epson", "brother", "canon mx", "pixma"]) {
        return "printer";
    }
    if any(&all, &["camera", "doorbell", "-cam", "cam-", "ipcam", "reolink", "hikvision", "dahua"]) {
        return "camera";
    }
    if any(&all, &["synology", "diskstation", "qnap", "truenas", "unraid", "nas"]) {
        return "nas";
    }
    if any(
        &all,
        &[
            "chromecast",
            "appletv",
            "apple-tv",
            "apple tv",
            "roku",
            "firetv",
            "fire tv",
            "shield",
            "bravia",
            "webos",
            "-tv",
            "tv-",
            " tv",
        ],
    ) {
        return "tv";
    }
    if any(&svc, &["_googlecast.", "_airplay.", "_androidtvremote", "upnp:mediarenderer", "_roku"]) {
        return if any(&svc, &["_raop.", "_spotify-connect."]) && !svc.contains("_airplay.") {
            "speaker"
        } else {
            "tv"
        };
    }
    if any(&svc, &["_raop.", "_spotify-connect.", "_sonos."]) || any(&vendor, &["sonos", "bose"]) {
        return "speaker";
    }
    if any(&svc, &["_hap.", "_homekit.", "_hue.", "_esphomelib.", "_matter", "_shelly"])
        || any(
            &vendor,
            &["espressif", "tuya", "shelly", "signify", "philips lighting", "lifx", "ring ", "nest", "ecobee", "wiz"],
        )
        || any(
            &all,
            &["esp-", "esp_", "shelly", "tasmota", "hue", "plug", "bulb", "lamp", "light", "sensor", "thermostat"],
        )
    {
        return "iot";
    }
    if any(&vendor, &["synology", "qnap", "western digital", "asustor", "buffalo"]) {
        return "nas";
    }
    if any(&vendor, &["hikvision", "dahua", "axis comm", "reolink", "amcrest"]) {
        return "camera";
    }
    if any(&vendor, &["sony interactive", "nintendo"]) {
        return "console";
    }
    if any(&vendor, &["roku", "lg electronics", "tcl", "hisense", "vizio"]) {
        return "tv";
    }
    if any(&vendor, &["seiko epson", "brother", "lexmark", "xerox", "kyocera"]) {
        return "printer";
    }
    if any(&vendor, &["ubiquiti", "ruckus", "aruba"]) || any(&svc, &["upnp:wlanaccesspoint"]) {
        return "ap";
    }
    if any(&svc, &["upnp:mediaserver"]) {
        return "server";
    }
    // Tailscale reports the OS.
    match os.as_str() {
        "ios" => return if any(&all, &["ipad"]) { "tablet" } else { "phone" },
        "android" => return "phone",
        "macos" => return "laptop",
        "windows" => return "desktop",
        "linux" | "freebsd" => return "server",
        _ => {}
    }
    if any(&svc, &["_smb.", "_afpovertcp.", "_adisk.", "_nfs.", "nfs share", "smb share"]) {
        return "nas";
    }
    if any(&svc, &["_ssh.", "_sftp-ssh.", "tcp/22"]) || vendor.contains("raspberry") {
        return "server";
    }
    if any(&svc, &["_workstation.", "_companion-link."]) {
        return "desktop";
    }
    if vendor == "random mac" {
        // Phones and laptops use private Wi-Fi addresses.
        return "phone";
    }
    "other"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(ip: &str) -> Device {
        Device {
            ip: ip.into(),
            mac: None,
            names: vec![],
            services: vec![],
            seen: vec![Seen::Neighbour],
            os: None,
            online: None,
            vendor: None,
            also: vec![],
            kind: "other".into(),
            model: None,
            routed: false,
            first_seen: None,
            is_new: false,
            trusted: false,
            link: None,
        }
    }

    #[test]
    fn parses_name_sources() {
        assert_eq!(
            parse_names(&[
                "192.168.1.20 printer.lan.",
                "192.168.1.21 192-168-1-21.isp.example",
                "junk",
                "192.168.1.22 *"
            ]),
            [("192.168.1.20".to_string(), "printer.lan".to_string())]
        );
        let l = parse_leases(&[
            "dnsmasq 1794860000 AA:BB:CC:DD:EE:01 192.168.1.30 living-room-lamp 01:aa:bb",
            "dnsmasq 1794860000 aa:bb:cc:dd:ee:02 192.168.1.31 * *",
            "isc 192.168.1.32 aa:bb:cc:dd:ee:03 office-printer",
        ]);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0], ("192.168.1.30".into(), Some("aa:bb:cc:dd:ee:01".into()), "living-room-lamp".into()));
        assert_eq!(l[1].2, "office-printer");
        let s = parse_ssdp(&["192.168.1.40\tLinux UPnP/1.0 Sonos/72\tKitchen\tSonos, Inc.\tSonos One\turn:schemas-upnp-org:device:ZonePlayer:1"]);
        assert_eq!(s[0].friendly, "Kitchen");
        assert_eq!(s[0].device_type, "ZonePlayer");
    }

    #[test]
    fn guesses_kinds() {
        let mut d = dev("192.168.1.1");
        assert_eq!(guess_kind(&d, Some("192.168.1.1")), "router");
        d = dev("192.168.1.30");
        d.names = vec!["living-room-lamp".into()];
        assert_eq!(guess_kind(&d, None), "iot");
        d = dev("192.168.1.31");
        d.vendor = Some("Espressif Inc.".into());
        assert_eq!(guess_kind(&d, None), "iot");
        d = dev("192.168.1.32");
        d.services = vec!["_ipp._tcp".into()];
        assert_eq!(guess_kind(&d, None), "printer");
        d = dev("192.168.1.33");
        d.services = vec!["_googlecast._tcp".into()];
        assert_eq!(guess_kind(&d, None), "tv");
        d = dev("192.168.1.34");
        d.names = vec!["Alex's iPhone".into()];
        assert_eq!(guess_kind(&d, None), "phone");
        d = dev("192.168.1.35");
        d.vendor = Some("Synology Incorporated".into());
        assert_eq!(guess_kind(&d, None), "nas");
        d = dev("192.168.1.36");
        d.services = vec!["tcp/22".into()];
        assert_eq!(guess_kind(&d, None), "server");
        d = dev("100.64.0.9");
        d.os = Some("iOS".into());
        assert_eq!(guess_kind(&d, None), "phone");
        assert_eq!(guess_kind(&dev("192.168.1.99"), None), "other");
    }
}

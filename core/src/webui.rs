//! Web UIs of services and containers, opened in a Nightgrid window (D-147).
//!
//! This module only decides *what* to open: which published ports of a
//! container look like a web interface, and the URL for one. How the app
//! reaches it (straight from this computer, or through an SSH tunnel to the
//! host) is decided in the app crate, which can dial.

use serde::Serialize;

use crate::docker::PortView;

/// Container label that names the web UI explicitly: a port (`8080`), a port
/// with scheme and path (`https:8443/admin`), or just a path (`/ui`) for the
/// first published web port.
pub const LABEL_WEBUI: &str = "nightgrid.webui";

/// One way into a container's web UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WebUiTarget {
    /// The port published on the host.
    pub port: u16,
    /// "http" or "https".
    pub scheme: String,
    /// Starts with "/".
    pub path: String,
    /// The address Docker bound the port to, when not every address.
    pub bind: Option<String>,
    /// Picked by the `nightgrid.webui` label rather than guessed.
    pub labelled: bool,
}

/// Ports that serve something other than a web page. A published port on
/// this list is never offered as a web UI unless a label says so.
const NOT_WEB: &[u16] = &[
    21, 22, 23, 25, 53, 67, 68, 69, 110, 123, 137, 138, 139, 143, 161, 389, 445, 465, 514, 587, 636, 993, 995, 1194,
    1433, 1521, 1883, 2049, 3306, 3478, 3389, 5353, 5432, 5672, 5900, 5901, 6379, 6881, 7777, 8883, 9042, 9092, 11211,
    19132, 25565, 25575, 26900, 27015, 27016, 27017, 27018, 28015, 34197, 51820,
];

/// Ports that usually speak TLS.
const TLS_PORTS: &[u16] = &[443, 2083, 5001, 8006, 8443, 9443, 10443];

/// Ports most likely to be the web UI, best first.
const LIKELY_WEB: &[u16] =
    &[80, 443, 8080, 8443, 9443, 3000, 8000, 8081, 8096, 9000, 5000, 5001, 8888, 8123, 32400, 7878, 8989, 9696, 8006];

/// The published TCP ports of a container that may be a web UI, best first.
/// `label` is the container's `nightgrid.webui` label, if any.
pub fn targets(ports: &[PortView], label: Option<&str>) -> Vec<WebUiTarget> {
    let mut out: Vec<WebUiTarget> = Vec::new();
    let published: Vec<&PortView> = ports.iter().filter(|p| p.proto == "tcp" && p.public.is_some()).collect();
    let bind_of = |p: &PortView| p.ip.clone().filter(|ip| !matches!(ip.as_str(), "0.0.0.0" | "::" | ""));
    let guess = |p: &PortView, path: &str, labelled: bool| WebUiTarget {
        port: p.public.unwrap_or(p.private),
        scheme: if TLS_PORTS.contains(&p.private) || TLS_PORTS.contains(&p.public.unwrap_or(0)) {
            "https"
        } else {
            "http"
        }
        .to_owned(),
        path: path.to_owned(),
        bind: bind_of(p),
        labelled,
    };

    let mut label_path = "/".to_owned();
    if let Some(l) = label.map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(path) = l.strip_prefix('/') {
            label_path = format!("/{path}");
        } else if let Some((scheme, port, path)) = parse_label(l) {
            // The label names the container port or the published one.
            let p = published
                .iter()
                .find(|p| p.private == port)
                .or_else(|| published.iter().find(|p| p.public == Some(port)));
            if let Some(p) = p {
                let mut t = guess(p, &path, true);
                if let Some(s) = scheme {
                    t.scheme = s;
                }
                out.push(t);
            }
        }
    }

    let mut rest: Vec<&PortView> = published.iter().copied().filter(|p| !NOT_WEB.contains(&p.private)).collect();
    let rank = |p: &PortView| LIKELY_WEB.iter().position(|w| *w == p.private).unwrap_or(LIKELY_WEB.len());
    rest.sort_by_key(|p| (rank(p), p.private));
    let labelled_path = out.is_empty() && label_path != "/";
    for (i, p) in rest.into_iter().enumerate() {
        let t = guess(p, if labelled_path && i == 0 { &label_path } else { "/" }, labelled_path && i == 0);
        if !out.iter().any(|o| o.port == t.port) {
            out.push(t);
        }
    }
    out
}

/// `8080`, `https:8443`, `8080/admin`, `https:8443/admin`.
fn parse_label(l: &str) -> Option<(Option<String>, u16, String)> {
    let (scheme, rest) = match l.split_once(':') {
        Some((s, r)) if matches!(s, "http" | "https") => (Some(s.to_owned()), r.trim_start_matches("//")),
        _ => (None, l),
    };
    let (port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    Some((scheme, port.parse().ok()?, path.to_owned()))
}

/// The URL of `t` on `host` (a name or an address).
pub fn url_for(t: &WebUiTarget, host: &str) -> String {
    let host = t.bind.as_deref().unwrap_or(host);
    let default = (t.scheme == "http" && t.port == 80) || (t.scheme == "https" && t.port == 443);
    let port = if default { String::new() } else { format!(":{}", t.port) };
    format!("{}://{}{port}{}", t.scheme, bracket(host), t.path)
}

fn bracket(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    }
}

/// An http(s) URL taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebUrl {
    pub scheme: String,
    /// Without IPv6 brackets.
    pub host: String,
    pub port: u16,
    /// Path, query and fragment; starts with "/".
    pub rest: String,
}

impl WebUrl {
    /// Same URL on another host and port (for the local end of a tunnel).
    pub fn with_endpoint(&self, host: &str, port: u16) -> String {
        format!("{}://{}:{port}{}", self.scheme, bracket(host), self.rest)
    }

    pub fn is_loopback(&self) -> bool {
        let h = self.host.to_ascii_lowercase();
        h == "localhost" || h.starts_with("127.") || h == "::1"
    }
}

/// Parses an http(s) URL. No user info, since a password in a URL would end up
/// in window titles and tunnel names.
pub fn parse_url(url: &str) -> Result<WebUrl, String> {
    let url = url.trim();
    let (scheme, rest) = url.split_once("://").ok_or("Enter a full address, like http://example.com:8080")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("Only http and https addresses open in Nightgrid.".into());
    }
    let split = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(split);
    if authority.contains('@') {
        return Err("Leave the user name and password out of the address; the page asks for them.".into());
    }
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (h, after) = v6.split_once(']').ok_or("Unclosed [ in the address.")?;
        (h.to_owned(), after.strip_prefix(':').filter(|p| !p.is_empty()))
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_owned(), Some(p)),
            None => (authority.to_owned(), None),
        }
    };
    if host.is_empty() || host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("The address has no host name.".into());
    }
    let port = match port {
        Some(p) => p.parse::<u16>().ok().filter(|p| *p > 0).ok_or("The port must be a number from 1 to 65535.")?,
        None if scheme == "https" => 443,
        None => 80,
    };
    let rest = if tail.starts_with('/') { tail.to_owned() } else { format!("/{tail}") };
    Ok(WebUrl { scheme, host, port, rest })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(private: u16, public: Option<u16>, ip: Option<&str>) -> PortView {
        PortView { ip: ip.map(str::to_owned), private, public, proto: "tcp".into() }
    }

    #[test]
    fn picks_web_ports_best_first_and_skips_others() {
        let ports = vec![
            port(25565, Some(25565), None),
            port(9000, Some(9000), None),
            port(8080, Some(8081), None),
            port(9443, Some(9443), None),
            port(3306, Some(3306), None),
            port(1234, None, None),
            PortView { ip: None, private: 8000, public: Some(8000), proto: "udp".into() },
        ];
        let t = targets(&ports, None);
        let got: Vec<(u16, &str)> = t.iter().map(|t| (t.port, t.scheme.as_str())).collect();
        assert_eq!(got, vec![(8081, "http"), (9443, "https"), (9000, "http")]);
    }

    #[test]
    fn label_names_port_scheme_and_path() {
        let ports = vec![port(80, Some(8080), None), port(8443, Some(18443), Some("127.0.0.1"))];
        let t = targets(&ports, Some("https:8443/admin"));
        assert_eq!(
            t[0],
            WebUiTarget {
                port: 18443,
                scheme: "https".into(),
                path: "/admin".into(),
                bind: Some("127.0.0.1".into()),
                labelled: true
            }
        );
        assert_eq!(t.len(), 2);
        assert_eq!(targets(&ports, Some("/panel"))[0].path, "/panel");
        assert_eq!(targets(&ports, Some("8080"))[0].port, 8080, "published port works too");
        assert_eq!(targets(&ports, Some("nonsense"))[0].port, 8080);
        // A label can open a port the guess would skip.
        let game = vec![port(25565, Some(25565), None)];
        assert!(targets(&game, None).is_empty());
        assert_eq!(targets(&game, Some("25565"))[0].port, 25565);
    }

    #[test]
    fn builds_urls() {
        let t = WebUiTarget { port: 8080, scheme: "http".into(), path: "/".into(), bind: None, labelled: false };
        assert_eq!(url_for(&t, "nas.example.com"), "http://nas.example.com:8080/");
        assert_eq!(url_for(&t, "fd00::5"), "http://[fd00::5]:8080/");
        let t = WebUiTarget {
            port: 443,
            scheme: "https".into(),
            path: "/x".into(),
            bind: Some("127.0.0.1".into()),
            labelled: false,
        };
        assert_eq!(url_for(&t, "nas"), "https://127.0.0.1/x");
    }

    #[test]
    fn parses_urls() {
        let u = parse_url("HTTP://192.168.1.5:8080/panel?x=1").unwrap();
        assert_eq!(
            (u.scheme.as_str(), u.host.as_str(), u.port, u.rest.as_str()),
            ("http", "192.168.1.5", 8080, "/panel?x=1")
        );
        assert_eq!(u.with_endpoint("127.0.0.1", 50123), "http://127.0.0.1:50123/panel?x=1");
        let u = parse_url("https://[fd00::1]/").unwrap();
        assert_eq!((u.host.as_str(), u.port), ("fd00::1", 443));
        let u = parse_url("http://example.com?q").unwrap();
        assert_eq!((u.port, u.rest.as_str()), (80, "/?q"));
        assert!(parse_url("http://localhost:3000").unwrap().is_loopback());
        assert!(parse_url("ftp://x").is_err());
        assert!(parse_url("example.com").is_err());
        assert!(parse_url("http://user:pw@example.com/").is_err());
        assert!(parse_url("http://example.com:0/").is_err());
        assert!(parse_url("http://:80/").is_err());
    }
}

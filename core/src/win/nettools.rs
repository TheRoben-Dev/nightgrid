//! The network tools (ping, path, DNS, ports, HTTP/TLS, neighbours,
//! connections, services) in PowerShell, for Windows hosts and for this
//! computer when Nightgrid itself runs on Windows. Each script prints what
//! the Linux tools print (or the same markers), so `nettools::parse` reads
//! both and the results look the same.

use crate::nettools::{
    parse, parse_ports, parse_url, timeout_secs, valid_host, Tool, ToolRequest, ToolResult, CERT, DNS_TYPES, NOTE,
    PORT, SERVICE_PORTS, SVC, WEB,
};

use super::quote;

fn bad(field: &str, why: &str) -> String {
    format!("{field}: {why}")
}

fn list(ports: &[u16]) -> String {
    ports.iter().map(u16::to_string).collect::<Vec<_>>().join(",")
}

/// The PowerShell for `r`, and what it runs in words.
pub fn script(r: &ToolRequest) -> Result<(String, String), String> {
    let target = r.target.trim();
    let need_host = || -> Result<String, String> {
        if valid_host(target) {
            Ok(quote(target))
        } else {
            Err(bad("target", "a host name or address"))
        }
    };
    Ok(match r.tool {
        Tool::Ping => {
            let t = need_host()?;
            let n: u8 = if r.option.trim().is_empty() { 4 } else { r.option.trim().parse().map_err(|_| bad("count", "1 to 10"))? };
            if !(1..=10).contains(&n) {
                return Err(bad("count", "1 to 10"));
            }
            (
                format!(
                    r#"$p = New-Object Net.NetworkInformation.Ping; $ok = @()
for ($i = 0; $i -lt {n}; $i++) {{
  try {{ $r = $p.Send({t}, 2000) }} catch {{ [Console]::Out.WriteLine('ping: ' + $_.Exception.InnerException.Message); break }}
  if ($r.Status -eq 'Success') {{ $ok += $r.RoundtripTime; [Console]::Out.WriteLine(('Reply from {{0}}: time={{1}} ms' -f $r.Address, $r.RoundtripTime)) }} else {{ [Console]::Out.WriteLine('No reply: ' + $r.Status) }}
  if ($i -lt {n} - 1) {{ Start-Sleep -Milliseconds 500 }}
}}
$loss = [math]::Round(100 * ({n} - $ok.Count) / {n})
[Console]::Out.WriteLine(('{n} packets transmitted, {{0}} received, {{1}}% packet loss' -f $ok.Count, $loss))
if ($ok.Count) {{ $m = $ok | Measure-Object -Minimum -Maximum -Average; [Console]::Out.WriteLine(('rtt min/avg/max/mdev = {{0}}/{{1:0.0}}/{{2}}/0 ms' -f $m.Minimum, $m.Average, $m.Maximum)) }}"#
                ),
                format!("ping {target} ({n} times)"),
            )
        }
        Tool::Path => {
            let t = need_host()?;
            (format!("& tracert.exe -d -h 20 -w 1000 {t} 2>&1 | ForEach-Object {{ [Console]::Out.WriteLine($_) }}"), format!("tracert {target}"))
        }
        Tool::Dns => {
            let t = need_host()?;
            let ty = r.option.trim().to_ascii_uppercase();
            let ty = if ty.is_empty() { "A".to_owned() } else { ty };
            if !DNS_TYPES.contains(&ty.as_str()) {
                return Err(bad("type", &format!("one of {}", DNS_TYPES.join(", "))));
            }
            let server = r.server.trim();
            if !server.is_empty() && !valid_host(server) {
                return Err(bad("server", "a host name or address"));
            }
            let srv = if server.is_empty() { String::new() } else { format!(" -Server {}", quote(server)) };
            (
                format!(
                    r#"try {{
  $a = @(Resolve-DnsName -Name {t} -Type {ty}{srv} -DnsOnly -ErrorAction Stop)
  [Console]::Out.WriteLine(';; ->>HEADER<<- opcode: QUERY, status: NOERROR, id: 0')
  foreach ($x in $a) {{
    if ($x.Section -and [string]$x.Section -ne 'Answer') {{ continue }}
    $d = switch ([string]$x.Type) {{
      'A' {{ $x.IPAddress }} 'AAAA' {{ $x.IPAddress }} 'MX' {{ '{{0}} {{1}}' -f $x.Preference, $x.NameExchange }}
      'TXT' {{ '"' + ($x.Strings -join '" "') + '"' }} 'SRV' {{ '{{0}} {{1}} {{2}} {{3}}' -f $x.Priority, $x.Weight, $x.Port, $x.NameTarget }}
      'SOA' {{ '{{0}} {{1}} {{2}}' -f $x.PrimaryServer, $x.NameAdministrator, $x.SerialNumber }}
      default {{ $x.NameHost }}
    }}
    [Console]::Out.WriteLine(('{{0}}. {{1}} IN {{2}} {{3}}' -f $x.Name, $x.TTL, $x.Type, $d))
  }}
}} catch {{
  $s = if ($_.Exception.Message -match 'does not exist') {{ 'NXDOMAIN' }} else {{ 'SERVFAIL' }}
  [Console]::Out.WriteLine(';; ->>HEADER<<- opcode: QUERY, status: ' + $s + ', id: 0')
  [Console]::Out.WriteLine($_.Exception.Message)
}}"#
                ),
                format!("{ty} {target}{}", if server.is_empty() { String::new() } else { format!(" @{server}") }),
            )
        }
        Tool::Ports => {
            let t = need_host()?;
            let ports = parse_ports(&r.option)?;
            (
                format!(
                    r#"foreach ($p in @({l})) {{
  $c = New-Object Net.Sockets.TcpClient
  $s = 'closed'
  try {{ if ($c.ConnectAsync({t}, $p).Wait(2000) -and $c.Connected) {{ $s = 'open' }} }} catch {{ }}
  $c.Close()
  [Console]::Out.WriteLine('{PORT} ' + $p + ' ' + $s)
}}"#,
                    l = list(&ports)
                ),
                format!("TCP connect to {target} on {} ports", ports.len()),
            )
        }
        Tool::Http => {
            let (tls, host, port) = parse_url(target)?;
            let u = quote(target);
            let h = quote(&host);
            let cert = if tls {
                format!(
                    r#"
$verify = 1
try {{
  $tc = New-Object Net.Sockets.TcpClient
  if ($tc.ConnectAsync({h}, {port}).Wait(5000)) {{
    $ssl = New-Object Net.Security.SslStream($tc.GetStream(), $false, {{ param($a, $b, $c, $e) $script:sslErr = $e; $true }})
    $ssl.AuthenticateAsClient({h})
    $x = New-Object Security.Cryptography.X509Certificates.X509Certificate2($ssl.RemoteCertificate)
    $verify = if ([string]$script:sslErr -eq 'None') {{ 0 }} else {{ [int]$script:sslErr }}
    [Console]::Out.WriteLine('{CERT}')
    [Console]::Out.WriteLine('subject=' + $x.Subject)
    [Console]::Out.WriteLine('issuer=' + $x.Issuer)
    [Console]::Out.WriteLine('notAfter=' + $x.NotAfter.ToUniversalTime().ToString('MMM d HH:mm:ss yyyy', [Globalization.CultureInfo]::InvariantCulture) + ' GMT')
    $ssl.Dispose()
  }}
  $tc.Close()
}} catch {{ [Console]::Out.WriteLine('{NOTE} no certificate details: ' + $_.Exception.Message) }}"#
                )
            } else {
                "\n$verify = 0".to_owned()
            };
            (
                format!(
                    r#"[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls11 -bor [Net.SecurityProtocolType]::Tls
[Net.ServicePointManager]::ServerCertificateValidationCallback = {{ $true }}{cert}
$ip = ''; try {{ $ip = [string]([Net.Dns]::GetHostAddresses({h}) | Select-Object -First 1) }} catch {{ }}
$sw = [Diagnostics.Stopwatch]::StartNew(); $code = '000'
try {{
  $q = [Net.HttpWebRequest]::Create({u}); $q.AllowAutoRedirect = $false; $q.Timeout = 10000; $q.UserAgent = 'Nightgrid'
  try {{ $resp = $q.GetResponse() }} catch [Net.WebException] {{ $resp = $_.Exception.Response; if (-not $resp) {{ [Console]::Out.WriteLine($_.Exception.Message) }} }}
  if ($resp) {{
    $code = [string][int]$resp.StatusCode
    [Console]::Out.WriteLine('HTTP/' + $resp.ProtocolVersion + ' ' + $code + ' ' + $resp.StatusDescription)
    foreach ($k in $resp.Headers.AllKeys) {{ [Console]::Out.WriteLine($k + ': ' + $resp.Headers[$k]) }}
    $resp.Close()
  }}
}} catch {{ [Console]::Out.WriteLine($_.Exception.Message) }}
[Console]::Out.WriteLine(('{WEB} {{0}} {{1:0.000}} {{2}} {{3}}' -f $code, $sw.Elapsed.TotalSeconds, $verify, $ip))"#
                ),
                format!("GET {target}"),
            )
        }
        Tool::Neighbours => (
            r#"Get-NetNeighbor -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.State -ne 'Unreachable' -and $_.LinkLayerAddress -and $_.LinkLayerAddress -notmatch '^(00-00-00-00-00-00|FF-FF-FF-FF-FF-FF|01-00-5E)' } | ForEach-Object {
  [Console]::Out.WriteLine(('{0} dev {1} lladdr {2} {3}' -f $_.IPAddress, ($_.InterfaceAlias -replace '\s', '_'), ($_.LinkLayerAddress -replace '-', ':').ToLower(), ([string]$_.State).ToUpper()))
}"#
            .into(),
            "Get-NetNeighbor".into(),
        ),
        Tool::Connections => (
            r#"function A($a, $p) { if ($a -match ':') { '[' + $a + ']:' + $p } else { $a + ':' + $p } }
Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue | Select-Object -First 400 | ForEach-Object {
  [Console]::Out.WriteLine(('tcp ESTAB 0 0 {0} {1}' -f (A $_.LocalAddress $_.LocalPort), (A $_.RemoteAddress $_.RemotePort)))
}"#
            .into(),
            "Get-NetTCPConnection".into(),
        ),
        Tool::Services => {
            let t = need_host()?;
            let ports = if r.option.trim().is_empty() { SERVICE_PORTS.to_vec() } else { parse_ports(&r.option)? };
            (
                format!(
                    r#"$tlsPorts = @(443, 465, 636, 853, 993, 995, 2376, 5986, 6443, 8006, 8443, 9443)
function Banner($s) {{
  $s.ReadTimeout = 1500; $buf = New-Object byte[] 512; $n = 0
  try {{ $n = $s.Read($buf, 0, 512) }} catch {{ }}
  if ($n -le 0) {{
    try {{ $w = [Text.Encoding]::ASCII.GetBytes("HEAD / HTTP/1.0`r`nHost: {host}`r`n`r`n"); $s.Write($w, 0, $w.Length); $n = $s.Read($buf, 0, 512) }} catch {{ $n = 0 }}
  }}
  if ($n -le 0) {{ return '' }}
  (([Text.Encoding]::ASCII.GetString($buf, 0, $n) -split "`n")[0] -replace '[^\x20-\x7e]', '').Trim()
}}
foreach ($p in @({l})) {{
  $c = New-Object Net.Sockets.TcpClient
  try {{ $up = $c.ConnectAsync({t}, $p).Wait(1500) -and $c.Connected }} catch {{ $up = $false }}
  if ($up) {{
    $tls = 0; $b = ''
    if ($tlsPorts -contains $p) {{
      try {{
        $ssl = New-Object Net.Security.SslStream($c.GetStream(), $false, {{ $true }})
        $ssl.AuthenticateAsClient({t}); $tls = 1; $b = Banner $ssl
      }} catch {{ $b = '' }}
    }} else {{ $b = Banner $c.GetStream() }}
    [Console]::Out.WriteLine("{SVC}`t$p`topen`t$tls`t$b")
  }}
  $c.Close()
}}"#,
                    l = list(&ports),
                    host = target.replace(['"', '`', '$'], ""),
                ),
                format!("service scan of {target} ({} ports)", ports.len()),
            )
        }
    })
}

/// Runs a tool on a Windows host over SSH.
pub async fn run(conn: &crate::ssh::client::Connection, r: &ToolRequest) -> Result<ToolResult, String> {
    let (ps, ran) = script(r)?;
    let o = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs(r.tool)), super::run(conn, &ps))
        .await
        .map_err(|_| "the tool took too long".to_string())??;
    let text = if o.stdout.trim().is_empty() { o.error_text() } else { o.stdout };
    Ok(parse(r, ran, &text, now()))
}

/// Runs a tool on this computer, when it is Windows.
#[cfg(windows)]
pub async fn run_local(r: &ToolRequest) -> Result<ToolResult, String> {
    let (ps, ran) = script(r)?;
    let work = tokio::task::spawn_blocking(move || super::run_local(&ps, b""));
    let o = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs(r.tool)), work)
        .await
        .map_err(|_| "the tool took too long".to_string())?
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let text = if o.stdout.trim().is_empty() { o.error_text() } else { o.stdout };
    Ok(parse(r, ran, &text, now()))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nettools::Tone;

    fn req(tool: Tool, target: &str, option: &str) -> ToolRequest {
        ToolRequest { tool, target: target.into(), option: option.into(), server: String::new() }
    }

    #[test]
    fn validates_like_linux() {
        assert!(script(&req(Tool::Ping, "-x", "")).is_err());
        assert!(script(&req(Tool::Ping, "host", "11")).is_err());
        assert!(script(&req(Tool::Dns, "example.com", "BOGUS")).is_err());
        assert!(script(&req(Tool::Ports, "h", "0")).is_err());
        assert!(script(&req(Tool::Http, "ftp://x", "")).is_err());
        let (s, _) = script(&req(Tool::Ports, "nas.lan", "22,80")).unwrap();
        assert!(s.contains("@(22,80)") && s.contains("'nas.lan'"));
    }

    #[test]
    fn output_reads_like_linux() {
        let r = req(Tool::Ping, "1.1.1.1", "2");
        let out = parse(&r, "x".into(), "Reply from 1.1.1.1: time=12 ms\nReply from 1.1.1.1: time=14 ms\n2 packets transmitted, 2 received, 0% packet loss\nrtt min/avg/max/mdev = 12/13.0/14/0 ms\n", 0);
        assert!(out.facts.iter().any(|f| f.label == "Loss" && f.tone == Tone::Ok));
        assert!(out.facts.iter().any(|f| f.label == "Round trip" && f.value.starts_with("13.0 ms")));
        let r = req(Tool::Dns, "example.com", "MX");
        let out = parse(
            &r,
            "x".into(),
            ";; ->>HEADER<<- opcode: QUERY, status: NOERROR, id: 0\nexample.com. 300 IN MX 10 mail.example.com\n",
            0,
        );
        assert_eq!(out.table.unwrap().rows[0], vec!["example.com", "MX", "10 mail.example.com", "300"]);
        let r = req(Tool::Neighbours, "", "");
        let out = parse(&r, "x".into(), "192.168.1.1 dev Ethernet lladdr aa:bb:cc:dd:ee:ff REACHABLE\n", 0);
        assert_eq!(out.table.unwrap().rows[0][1], "aa:bb:cc:dd:ee:ff");
        let r = req(Tool::Connections, "", "");
        let out = parse(&r, "x".into(), "tcp ESTAB 0 0 10.0.0.5:22 10.0.0.9:51000\n", 0);
        assert_eq!(out.table.unwrap().rows.len(), 1);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn runs_locally() {
        let out = run_local(&req(Tool::Ping, "127.0.0.1", "2")).await.unwrap();
        assert!(out.facts.iter().any(|f| f.label == "Loss" && f.value == "0%"), "{:?}\n{}", out.facts, out.output);
        let out = run_local(&req(Tool::Connections, "", "")).await.unwrap();
        assert!(out.note.is_none());
        let out = run_local(&req(Tool::Dns, "localhost", "A")).await.unwrap();
        assert!(!out.output.is_empty());
    }
}

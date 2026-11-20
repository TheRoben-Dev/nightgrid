//! Fleet → SPEED on Windows hosts, with `iperf3.exe` (Windows has no
//! iperf3 of its own and Nightgrid installs nothing, so it has to be on the
//! PATH or in `C:\iperf3`). Processes started in an SSH session die with
//! it on Windows, so the server and its watchdog are started through
//! `Win32_Process.Create`, which runs them outside the session. The client
//! side prints the same `@@ping` / `@@up` / `@@down` sections as the POSIX
//! script, with the ping summary in Linux words, so
//! [`crate::netperf::parse_client`] reads it.

use crate::netperf::SERVER_TTL;

use super::quote;

/// Prints `@@noiperf` and exits when iperf3.exe can't be found; else sets `$iperf`.
const FIND: &str = r#"$iperf = $null
$c = Get-Command iperf3.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if ($c) { $iperf = $c.Source } else {
  foreach ($p in @("$env:SystemDrive\iperf3\iperf3.exe", "$env:ProgramFiles\iperf3\iperf3.exe")) { if (Test-Path -LiteralPath $p) { $iperf = $p; break } }
}
if (-not $iperf) { [Console]::Out.WriteLine('@@noiperf'); exit 0 }
"#;

/// What to say when iperf3.exe is missing on `side` ("source" / "target").
pub fn missing(side: &str) -> String {
    format!(
        "iperf3 isn't on the {side} host. Windows has no iperf3 of its own: download iperf3 for Windows (iperf.fr) and put iperf3.exe on the PATH or in C:\\iperf3, then test again."
    )
}

/// Starts an iperf3 server on `port` and a watchdog that stops it after
/// [`SERVER_TTL`] seconds; prints `@@server <pid>`.
pub fn server_script(port: u16) -> String {
    format!(
        r#"$ErrorActionPreference = 'Stop'
{FIND}try {{
  $r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{ CommandLine = ('"' + $iperf + '" -s -p {port}') }}
  if ($r.ReturnValue -ne 0) {{ throw ('Win32_Process.Create returned ' + $r.ReturnValue) }}
  $id = [int]$r.ProcessId
  $dog = 'Start-Sleep -Seconds {SERVER_TTL}; Stop-Process -Id ' + $id + ' -Force -ErrorAction SilentlyContinue'
  [void](Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{ CommandLine = ('powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -Command "' + $dog + '"') }})
  Start-Sleep -Milliseconds 700
  if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) {{ [Console]::Out.WriteLine('@@err the iperf3 server did not start (is port {port} in use?)'); exit 0 }}
  [Console]::Out.WriteLine('@@server ' + $id + ' win')
}} catch {{ [Console]::Out.WriteLine('@@err ' + $_.Exception.Message) }}
"#
    )
}

/// Stops the server started by [`server_script`] (only if it is iperf3).
pub fn stop_script(pid: u32) -> String {
    format!("Get-Process -Id {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.ProcessName -eq 'iperf3' }} | Stop-Process -Force")
}

/// The server's pid from `@@server <pid> win`.
pub fn parse_server(out: &str) -> Result<u32, String> {
    let mut errs = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line == "@@noiperf" {
            return Err(missing("target"));
        }
        if let Some(rest) = line.strip_prefix("@@server ") {
            if let Some(pid) = rest.split_whitespace().next().and_then(|p| p.parse().ok()) {
                return Ok(pid);
            }
        }
        if let Some(e) = line.strip_prefix("@@err ") {
            errs.push(e.trim().to_owned());
        }
    }
    Err(if errs.is_empty() { "the iperf3 server didn't start".into() } else { errs.join("\n") })
}

/// Ping (5 echoes, summarised in Linux words), then iperf3 both ways.
pub fn client_script(target: &str, port: u16, secs: u8) -> String {
    let t = quote(target);
    let secs = secs.clamp(1, 30);
    format!(
        r#"$ErrorActionPreference = 'Continue'
{FIND}[Console]::Out.WriteLine('@@ping')
$ms = @()
$sent = 5
for ($i = 0; $i -lt $sent; $i++) {{
  $p = New-Object System.Net.NetworkInformation.Ping
  try {{ $r = $p.Send({t}, 2000); if ($r.Status -eq 'Success') {{ $ms += [double]$r.RoundtripTime }} }} catch {{ }}
  Start-Sleep -Milliseconds 200
}}
$got = $ms.Count
$loss = [int](100 * ($sent - $got) / $sent)
[Console]::Out.WriteLine(('{{0}} packets transmitted, {{1}} received, {{2}}% packet loss' -f $sent, $got, $loss))
if ($got) {{
  $avg = ($ms | Measure-Object -Average).Average
  $sd = [math]::Sqrt((($ms | ForEach-Object {{ ($_ - $avg) * ($_ - $avg) }}) | Measure-Object -Sum).Sum / $got)
  $inv = [Globalization.CultureInfo]::InvariantCulture
  [Console]::Out.WriteLine(('rtt min/avg/max/mdev = ' + (($ms | Measure-Object -Minimum).Minimum).ToString('0.000', $inv) + '/' + $avg.ToString('0.000', $inv) + '/' + (($ms | Measure-Object -Maximum).Maximum).ToString('0.000', $inv) + '/' + $sd.ToString('0.000', $inv) + ' ms'))
}}
Start-Sleep -Seconds 1
[Console]::Out.WriteLine('@@up')
$o = & $iperf -c {t} -p {port} -t {secs} -J 2>&1 | Out-String
[Console]::Out.WriteLine($o)
[Console]::Out.WriteLine('@@down')
$o = & $iperf -c {t} -p {port} -t {secs} -R -J 2>&1 | Out-String
[Console]::Out.WriteLine($o)
[Console]::Out.WriteLine('@@end')
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_lines() {
        assert_eq!(parse_server("@@server 4242 win\n"), Ok(4242));
        assert!(parse_server("@@noiperf").unwrap_err().contains("C:\\iperf3"));
        assert_eq!(parse_server("@@err port in use").unwrap_err(), "port in use");
    }

    #[test]
    fn scripts_are_ps51() {
        for s in [server_script(5201), client_script("10.0.0.2", 5201, 5), stop_script(1)] {
            assert!(!s.contains("??") && !s.contains("&&"), "{s}");
        }
        assert!(client_script("a'b", 5201, 5).contains("'a''b'"));
    }

    #[test]
    fn client_output_parses() {
        let out = "@@ping\n5 packets transmitted, 5 received, 0% packet loss\nrtt min/avg/max/mdev = 1.000/2.000/3.000/0.500 ms\n@@up\n{\"end\":{\"sum_received\":{\"bits_per_second\":9.4e8},\"sum_sent\":{\"retransmits\":2}}}\n@@down\n{\"end\":{\"sum_received\":{\"bits_per_second\":9.1e8}}}\n@@end\n";
        let t = crate::netperf::parse_client(1, 2, "x", 0, out);
        assert_eq!(t.rtt_ms, Some(2.0));
        assert_eq!(t.loss_pct, Some(0.0));
        assert_eq!(t.up_bps, Some(9.4e8));
        assert!(t.error.is_none(), "{:?}", t.error);
    }

    #[cfg(windows)]
    #[test]
    fn client_without_iperf_says_so() {
        // CI runners have no iperf3.exe: the script must say so cleanly.
        let o = super::super::run_local(&client_script("127.0.0.1", 5201, 1), b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(o.stdout.contains("@@noiperf") || o.stdout.contains("@@ping"), "{}", o.stdout);
    }
}

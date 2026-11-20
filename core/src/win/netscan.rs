//! Network discovery from a Windows host (or Nightgrid running on Windows):
//! the same sections the Linux script prints, read with Windows' own tools.
//! Interfaces and routes from Get-NetIPAddress/Get-NetRoute, a parallel ping
//! sweep and TCP probe in .NET, the neighbour table (Get-NetNeighbor),
//! reverse DNS, DHCP servers, live connections and Tailscale peers. mDNS,
//! NetBIOS, SSDP and UPnP port forwards aren't read on Windows.

use std::net::Ipv4Addr;

use crate::netmap::PROBE_PORTS;

const MARK: &str = "@@NGNET@@";

fn prefix_of(n: u32) -> u32 {
    32 - (n + 1).next_power_of_two().trailing_zeros()
}

/// The PowerShell for one discovery pass over `sweeps` (network, host count).
pub fn discover_script(sweeps: &[(Ipv4Addr, u32)]) -> String {
    let mut s = String::from(
        r#"$ErrorActionPreference = 'SilentlyContinue'
function W($l) { [Console]::Out.WriteLine($l) }
$found = New-Object 'System.Collections.Generic.HashSet[string]'
"#,
    );
    let sec = |s: &mut String, name: &str, body: &str| {
        s.push_str(&format!("W '{MARK} {name}'\n{body}\n"));
    };
    sec(
        &mut s,
        "addr",
        r#"$i = 0
Get-NetIPAddress | Where-Object { $_.AddressState -eq 'Preferred' } | ForEach-Object {
  $i++; $fam = if ($_.AddressFamily -eq 'IPv6') { 'inet6' } else { 'inet' }
  W ('{0}: {1} {2} {3}/{4} scope global' -f $i, ($_.InterfaceAlias -replace '\s', '_'), $fam, ($_.IPAddress -replace '%.*$', ''), $_.PrefixLength)
}"#,
    );
    sec(
        &mut s,
        "route",
        r#"Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | ForEach-Object { W ('default via {0} dev {1}' -f $_.NextHop, $_.InterfaceIndex) }
Get-NetRoute -DestinationPrefix '::/0' | Sort-Object RouteMetric | ForEach-Object { W ('default via {0} dev {1}' -f $_.NextHop, $_.InterfaceIndex) }"#,
    );
    for &(net, n) in sweeps {
        let base = u32::from(net);
        sec(
            &mut s,
            &format!("sweep {net}/{}", prefix_of(n)),
            &format!(
                r#"$base = {base}; $n = {n}
$ips = 1..$n | ForEach-Object {{ ([Net.IPAddress]([BitConverter]::GetBytes([uint32]($base + $_))[3..0] -join '.')).ToString() }}
$jobs = @(); foreach ($ip in $ips) {{ $jobs += [pscustomobject]@{{ ip = $ip; t = (New-Object Net.NetworkInformation.Ping).SendPingAsync($ip, 1000) }} }}
try {{ [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($jobs | ForEach-Object {{ $_.t }}), 5000) | Out-Null }} catch {{ }}
foreach ($j in $jobs) {{ if ($j.t.IsCompleted -and -not $j.t.IsFaulted -and $j.t.Result.Status -eq 'Success') {{ W $j.ip; [void]$found.Add($j.ip) }} }}"#
            ),
        );
        let ports = PROBE_PORTS.split_whitespace().collect::<Vec<_>>().join(",");
        sec(
            &mut s,
            "probe",
            &format!(
                r#"$probe = @(); foreach ($ip in $ips) {{ foreach ($p in @({ports})) {{ $c = New-Object Net.Sockets.TcpClient; $probe += [pscustomobject]@{{ ip = $ip; p = $p; c = $c; t = $c.ConnectAsync($ip, $p) }} }} }}
Start-Sleep -Milliseconds 1500
$hit = @{{}}
foreach ($x in $probe) {{ if (-not $hit[$x.ip] -and $x.t.IsCompleted -and -not $x.t.IsFaulted -and $x.c.Connected) {{ W ($x.ip + ' ' + $x.p); $hit[$x.ip] = 1; [void]$found.Add($x.ip) }}; $x.c.Close() }}"#
            ),
        );
    }
    for &(net, n) in sweeps {
        let first = Ipv4Addr::from(u32::from(net) + 1);
        sec(
            &mut s,
            &format!("path {net}/{}", prefix_of(n)),
            &format!(
                r#"& tracert.exe -d -h 6 -w 1000 {first} 2>$null | ForEach-Object {{ if ($_ -match '^\s*\d+\s.*\s(\d+\.\d+\.\d+\.\d+)\s*$') {{ W $Matches[1] }} }}"#
            ),
        );
    }
    sec(
        &mut s,
        "dhcp",
        r#"Get-CimInstance Win32_NetworkAdapterConfiguration -Filter 'DHCPEnabled = TRUE' | Where-Object { $_.DHCPServer } | ForEach-Object { W ('server_identifier ' + $_.DHCPServer) }"#,
    );
    sec(
        &mut s,
        "neigh",
        r#"Get-NetNeighbor -AddressFamily IPv4 | Where-Object { $_.State -ne 'Unreachable' -and $_.State -ne 'Incomplete' -and $_.LinkLayerAddress -and $_.LinkLayerAddress -notmatch '^(00-00-00-00-00-00|FF-FF-FF-FF-FF-FF|01-00-5E)' } | ForEach-Object {
  W ('{0} dev {1} lladdr {2} {3}' -f $_.IPAddress, ($_.InterfaceAlias -replace '\s', '_'), ($_.LinkLayerAddress -replace '-', ':').ToLower(), ([string]$_.State).ToUpper())
  [void]$found.Add([string]$_.IPAddress)
}"#,
    );
    sec(
        &mut s,
        "rdns",
        r#"$q = @(); foreach ($ip in @($found | Select-Object -First 600)) { $q += [pscustomobject]@{ ip = $ip; t = [Net.Dns]::GetHostEntryAsync($ip) } }
try { [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($q | ForEach-Object { $_.t }), 4000) | Out-Null } catch { }
foreach ($x in $q) { if ($x.t.IsCompleted -and -not $x.t.IsFaulted -and $x.t.Result.HostName -and $x.t.Result.HostName -ne $x.ip) { W ($x.ip + ' ' + $x.t.Result.HostName) } }"#,
    );
    sec(
        &mut s,
        "conns",
        r#"Get-NetTCPConnection -State Established | Select-Object -First 500 | ForEach-Object { if ($_.RemoteAddress -match ':') { W ('[' + $_.RemoteAddress + ']:' + $_.RemotePort) } else { W ($_.RemoteAddress + ':' + $_.RemotePort) } }"#,
    );
    sec(
        &mut s,
        "tailscale",
        r#"$ts = Get-Command tailscale.exe -ErrorAction SilentlyContinue
if (-not $ts) { $p = Join-Path $env:ProgramFiles 'Tailscale\tailscale.exe'; if (Test-Path $p) { $ts = $p } }
if ($ts) { $j = (& $ts status --json 2>$null) -join "`n"; if ($j.Length -gt 400000) { $j = $j.Substring(0, 400000) }; W $j } else { W 'missing' }"#,
    );
    sec(&mut s, "end", "");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_in_order() {
        let s = discover_script(&[("192.168.1.0".parse().unwrap(), 254)]);
        for name in ["addr", "route", "sweep 192.168.1.0/24", "probe", "path 192.168.1.0/24", "neigh", "rdns", "end"] {
            assert!(s.contains(&format!("W '{MARK} {name}'")), "{name}");
        }
        assert!(s.contains("$base = 3232235776; $n = 254"));
        assert!(!s.contains("mdns") && !s.contains("upnp"));
    }

    #[test]
    fn output_parses() {
        let d = crate::netmap::parse_discovery(
            "@@NGNET@@ addr\n1: Ethernet inet 192.168.1.5/24 scope global\n@@NGNET@@ route\ndefault via 192.168.1.1 dev 4\n\
             @@NGNET@@ sweep 192.168.1.0/24\n192.168.1.1\n@@NGNET@@ neigh\n192.168.1.20 dev Ethernet lladdr aa:bb:cc:dd:ee:ff REACHABLE\n\
             @@NGNET@@ rdns\n192.168.1.1 router.lan\n@@NGNET@@ end\n",
        );
        assert_eq!(d.gateway.as_deref(), Some("192.168.1.1"));
        assert_eq!(d.ifaces.len(), 1);
        assert!(d.devices.iter().any(|x| x.ip == "192.168.1.20"));
        assert!(d.devices.iter().any(|x| x.ip == "192.168.1.1" && x.names.iter().any(|n| n == "router.lan")));
    }

    #[cfg(windows)]
    #[test]
    fn runs_locally() {
        let o = super::super::run_local(&discover_script(&[]), b"").unwrap();
        let d = crate::netmap::parse_discovery(&o.stdout);
        assert!(!d.ifaces.is_empty(), "{}\n{}", o.stdout, o.stderr);
    }
}

//! Metrics for Windows hosts: a PowerShell sampler that stays running on
//! the host (one PowerShell start, not one per sample) and prints the same
//! `@@section`s as [`crate::metrics::COLLECT`], in the Linux formats the
//! parsers already read. CPU time comes from the raw processor counters,
//! memory from the memory counters and page files, network and disk bytes
//! from their raw (cumulative) performance counters, sizes from the local
//! volumes, temperatures from ACPI thermal zones where Windows exposes them
//! and GPUs from `nvidia-smi` when it's installed. Windows has no load
//! average, so that section stays empty.
//!
//! Each newline on stdin asks for one sample; end of input (or `q`) stops it.

use russh::ChannelMsg;

use crate::ssh::client::Connection;

pub const SAMPLER: &str = r#"$ErrorActionPreference = 'SilentlyContinue'
$nv = Get-Command nvidia-smi -ErrorAction SilentlyContinue
$tz = @(Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature).Count -gt 0
function J($v) { [uint64][math]::Floor([double]$v / 100000) }
function Sample {
  $b = New-Object Text.StringBuilder
  [void]$b.AppendLine('@@stat')
  $cpus = @(Get-CimInstance Win32_PerfRawData_PerfOS_Processor)
  $rows = @($cpus | Where-Object { $_.Name -eq '_Total' }) + @($cpus | Where-Object { $_.Name -ne '_Total' } | Sort-Object { [int]$_.Name })
  foreach ($c in $rows) {
    $n = 'cpu'
    if ($c.Name -ne '_Total') { $n = 'cpu' + $c.Name }
    # "% Processor Time" is an inverse timer: its raw value is idle time.
    [void]$b.AppendLine("$n $(J $c.PercentUserTime) 0 $(J $c.PercentPrivilegedTime) $(J $c.PercentProcessorTime) 0 0 0 0")
  }
  $os = Get-CimInstance Win32_OperatingSystem
  $mem = Get-CimInstance Win32_PerfRawData_PerfOS_Memory
  [void]$b.AppendLine('@@meminfo')
  [void]$b.AppendLine("MemTotal: $($os.TotalVisibleMemorySize) kB")
  [void]$b.AppendLine("MemFree: $($os.FreePhysicalMemory) kB")
  $av = $os.FreePhysicalMemory
  if ($mem.AvailableBytes) { $av = [uint64][math]::Floor([double]$mem.AvailableBytes / 1024) }
  [void]$b.AppendLine("MemAvailable: $av kB")
  $pt = 0; $pu = 0
  foreach ($p in @(Get-CimInstance Win32_PageFileUsage)) { $pt += $p.AllocatedBaseSize; $pu += $p.CurrentUsage }
  [void]$b.AppendLine("SwapTotal: $($pt * 1024) kB")
  [void]$b.AppendLine("SwapFree: $(($pt - $pu) * 1024) kB")
  [void]$b.AppendLine('@@loadavg')
  [void]$b.AppendLine('@@uptime')
  [void]$b.AppendLine([string][math]::Floor(((Get-Date) - $os.LastBootUpTime).TotalSeconds))
  [void]$b.AppendLine('@@kernel')
  [void]$b.AppendLine((("$($os.Caption) $($os.Version)") -replace '^Microsoft ', ''))
  [void]$b.AppendLine('@@hostname')
  [void]$b.AppendLine($env:COMPUTERNAME)
  [void]$b.AppendLine('@@netdev')
  foreach ($i in @(Get-CimInstance Win32_PerfRawData_Tcpip_NetworkInterface)) {
    $name = ($i.Name -replace ':', '_').Trim()
    [void]$b.AppendLine("${name}: $($i.BytesReceivedPersec) 0 0 0 0 0 0 0 $($i.BytesSentPersec) 0 0 0 0 0 0 0")
  }
  $disks = @(Get-CimInstance Win32_PerfRawData_PerfDisk_PhysicalDisk | Where-Object { $_.Name -ne '_Total' })
  $names = @()
  [void]$b.AppendLine('@@diskstats')
  foreach ($d in $disks) {
    $id = 'disk' + ($d.Name -split ' ')[0]
    $names += $id
    [void]$b.AppendLine("0 0 $id 0 0 $([uint64][math]::Floor([double]$d.DiskReadBytesPersec / 512)) 0 0 0 $([uint64][math]::Floor([double]$d.DiskWriteBytesPersec / 512))")
  }
  [void]$b.AppendLine('@@blocks')
  [void]$b.AppendLine(($names -join ' '))
  $vols = @(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3')
  [void]$b.AppendLine('@@mounts')
  foreach ($v in $vols) { [void]$b.AppendLine("$($v.DeviceID) $($v.DeviceID)\ $(([string]$v.FileSystem).ToLower())") }
  [void]$b.AppendLine('@@df')
  foreach ($v in $vols) {
    $t = [uint64][math]::Floor([double]$v.Size / 1024)
    $a = [uint64][math]::Floor([double]$v.FreeSpace / 1024)
    $u = $t - $a
    $pc = 0
    if ($t -gt 0) { $pc = [math]::Round($u * 100 / $t) }
    [void]$b.AppendLine("$($v.DeviceID) $t $u $a $pc% $($v.DeviceID)\")
  }
  [void]$b.AppendLine('@@hwmon')
  [void]$b.AppendLine('@@thermal')
  if ($tz) {
    foreach ($z in @(Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature)) {
      [void]$b.AppendLine("acpitz|$([int](([double]$z.CurrentTemperature / 10 - 273.15) * 1000))")
    }
  }
  [void]$b.AppendLine('@@nvidia')
  if ($nv) {
    foreach ($l in @(& $nv.Source '--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw' '--format=csv,noheader,nounits' 2>$null)) { [void]$b.AppendLine($l) }
  }
  [void]$b.AppendLine('@@end')
  [Console]::Out.Write($b.ToString())
  [Console]::Out.Flush()
}
while ($true) {
  $l = [Console]::In.ReadLine()
  if ($null -eq $l -or $l -eq 'q') { break }
  Sample
}
"#;

/// A sampler running on one Windows host.
pub struct Sampler {
    ch: russh::Channel<russh::client::Msg>,
}

impl Sampler {
    pub async fn start(conn: &Connection) -> Result<Self, String> {
        Ok(Self { ch: super::open(conn, SAMPLER).await? })
    }

    /// One sample's output (up to and including `@@end`).
    pub async fn sample(&mut self) -> Result<String, String> {
        self.ch.data(&b"\n"[..]).await.map_err(|e| format!("sampler: {e}"))?;
        let mut out = Vec::new();
        let mut err = Vec::new();
        loop {
            match self.ch.wait().await {
                Some(ChannelMsg::Data { data }) => {
                    out.extend_from_slice(&data);
                    if ends_sample(&out) {
                        return Ok(super::text(&out));
                    }
                }
                Some(ChannelMsg::ExtendedData { data, .. }) => err.extend_from_slice(&data),
                Some(ChannelMsg::Eof | ChannelMsg::Close) | None => {
                    let e = super::clean_stderr(&super::text(&err));
                    return Err(if e.is_empty() { "the PowerShell sampler stopped".into() } else { e });
                }
                Some(_) => {}
            }
        }
    }

    pub async fn stop(self) {
        let _ = self.ch.data(&b"q\n"[..]).await;
        let _ = self.ch.eof().await;
    }
}

fn ends_sample(out: &[u8]) -> bool {
    let t = String::from_utf8_lossy(out);
    t.trim_end().ends_with("@@end")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_end_is_found() {
        assert!(ends_sample(b"@@stat\r\ncpu 1 2\r\n@@end\r\n"));
        assert!(!ends_sample(b"@@stat\r\ncpu 1 2\r\n"));
    }

    /// Typical sampler output parses into a full snapshot.
    #[test]
    fn parses_windows_shaped_output() {
        let out = "@@stat\r\ncpu 100 0 50 850 0 0 0 0\r\ncpu0 50 0 25 425 0 0 0 0\r\ncpu1 50 0 25 425 0 0 0 0\r\n\
@@meminfo\r\nMemTotal: 16777216 kB\r\nMemFree: 4000000 kB\r\nMemAvailable: 8388608 kB\r\nSwapTotal: 2097152 kB\r\nSwapFree: 1048576 kB\r\n\
@@loadavg\r\n@@uptime\r\n3600\r\n@@kernel\r\nWindows Server 2022 Standard 10.0.20348\r\n@@hostname\r\nSRV01\r\n\
@@netdev\r\nIntel[R] Ethernet Connection I219-V: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0\r\n\
@@diskstats\r\n0 0 disk0 0 0 2048 0 0 0 4096\r\n@@blocks\r\ndisk0\r\n\
@@mounts\r\nC: C:\\ ntfs\r\nD: D:\\ refs\r\n@@df\r\nC: 104857600 52428800 52428800 50% C:\\\r\nD: 1048576 1048576 0 100% D:\\\r\n\
@@hwmon\r\n@@thermal\r\nacpitz|41850\r\n@@nvidia\r\n@@end\r\n";
        let r = crate::metrics::parse(out);
        assert_eq!(r.cores.len(), 2);
        assert_eq!(r.cpu.unwrap().idle, 850);
        assert_eq!(r.mem.total, 16 << 30);
        assert_eq!(r.mem.used(), 8 << 30);
        assert_eq!(r.uptime, 3600.0);
        assert_eq!(r.hostname, "SRV01");
        assert_eq!(r.kernel, "Windows Server 2022 Standard 10.0.20348");
        assert_eq!(r.net[0].name, "Intel[R] Ethernet Connection I219-V");
        assert_eq!((r.net[0].rx, r.net[0].tx), (1000, 2000));
        assert_eq!(r.disks[0].name, "disk0");
        assert_eq!(r.disks[0].read_sectors, 2048);
        let m: Vec<_> = r.mounts.iter().map(|m| (m.mount.as_str(), m.fstype.as_str(), m.total)).collect();
        assert_eq!(m, [("C:\\", "ntfs", 100 << 30), ("D:\\", "refs", 1 << 30)]);
        assert_eq!(r.temps[0].celsius, 41.9);
    }

    /// The real sampler on this machine (Windows CI) answers two requests
    /// with plausible numbers.
    #[cfg(windows)]
    #[test]
    fn sampler_runs_locally() {
        let o = super::super::run_local(SAMPLER, b"\n\n").unwrap();
        let parts: Vec<&str> = o.stdout.split("@@end").filter(|p| p.contains("@@stat")).collect();
        assert_eq!(parts.len(), 2, "{o:?}");
        let mut c = crate::metrics::Collector::new();
        let a = crate::metrics::parse(parts[0]);
        assert!(a.cpu.is_some() && !a.cores.is_empty(), "{}", parts[0]);
        assert!(a.mem.total > 0 && a.mem.used() > 0 && a.mem.used() < a.mem.total, "{}", parts[0]);
        assert!(!a.hostname.is_empty() && a.kernel.contains("Windows"), "{}", parts[0]);
        assert!(a.uptime > 0.0);
        assert!(!a.mounts.is_empty(), "{}", parts[0]);
        assert!(!a.disks.is_empty(), "{}", parts[0]);
        c.sample(a, 0.0);
        let s = c.sample(crate::metrics::parse(parts[1]), 1000.0);
        let cpu = s.cpu.expect("cpu % from two samples");
        assert!((0.0..=100.0).contains(&cpu), "{cpu}");
    }
}

//! Disk health on Windows hosts (System → DISKS, Fleet → DISKS and the
//! SMART alerts): physical disks from Windows' own Storage module
//! (`Get-PhysicalDisk`, plus `Get-StorageReliabilityCounter` for
//! temperature, wear, hours and errors, which Windows only shows to
//! administrators) and Storage Spaces pools. Nothing to install: no
//! smartctl.

use serde::Deserialize;

use crate::ssh::client::Connection;
use crate::system::pools::{Pool, PoolDevice, PoolKind, PoolsReport};
use crate::system::smart::{Disk, DisksReport, Health};
use crate::system::{SystemError, SystemResult};

use super::run;

const DISKS: &str = r#"$ErrorActionPreference = 'Stop'
$out = @(Get-PhysicalDisk | Sort-Object { [int]$_.DeviceId } | ForEach-Object {
  $r = $null; try { $r = $_ | Get-StorageReliabilityCounter -ErrorAction Stop } catch { }
  [ordered]@{
    id = [string]$_.DeviceId; model = [string]$_.FriendlyName; serial = ([string]$_.SerialNumber).Trim(); firmware = [string]$_.FirmwareVersion
    bus = [string]$_.BusType; media = [string]$_.MediaType; size = [int64]$_.Size; rpm = $_.SpindleSpeed
    health = [string]$_.HealthStatus; status = (@($_.OperationalStatus) -join ', ')
    temp = $(if ($r) { $r.Temperature } else { $null }); wear = $(if ($r) { $r.Wear } else { $null }); hours = $(if ($r) { $r.PowerOnHours } else { $null })
    readErr = $(if ($r) { $r.ReadErrorsUncorrected } else { $null }); writeErr = $(if ($r) { $r.WriteErrorsUncorrected } else { $null })
  }
})
ConvertTo-Json -InputObject $out -Depth 3 -Compress"#;

const POOLS: &str = r#"$ErrorActionPreference = 'Stop'
$out = @(Get-StoragePool -IsPrimordial $false | ForEach-Object {
  $p = $_
  [ordered]@{
    name = [string]$p.FriendlyName; health = [string]$p.HealthStatus; status = (@($p.OperationalStatus) -join ', ')
    size = [int64]$p.Size; used = [int64]$p.AllocatedSize
    disks = @($p | Get-PhysicalDisk | ForEach-Object { [ordered]@{ name = [string]$_.FriendlyName; health = [string]$_.HealthStatus; status = (@($_.OperationalStatus) -join ', ') } })
    vdisks = @($p | Get-VirtualDisk | ForEach-Object { [ordered]@{ name = [string]$_.FriendlyName; health = [string]$_.HealthStatus; status = (@($_.OperationalStatus) -join ', '); layout = [string]$_.ResiliencySettingName } })
    jobs = @(Get-StorageJob -ErrorAction SilentlyContinue | Where-Object { $_.JobState -eq 'Running' } | ForEach-Object { [ordered]@{ name = [string]$_.Name; pct = [int]$_.PercentComplete } })
  }
})
ConvertTo-Json -InputObject $out -Depth 4 -Compress"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDisk {
    id: String,
    model: Option<String>,
    serial: Option<String>,
    firmware: Option<String>,
    bus: String,
    media: String,
    size: Option<u64>,
    rpm: Option<u64>,
    health: String,
    status: String,
    temp: Option<i64>,
    wear: Option<i64>,
    hours: Option<u64>,
    read_err: Option<u64>,
    write_err: Option<u64>,
}

fn nonempty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn disk(r: RawDisk) -> Disk {
    let mut issues: Vec<(Health, String)> = Vec::new();
    match r.health.as_str() {
        "Healthy" => {}
        "Warning" => {
            issues.push((Health::Warn, format!("Windows reports this disk's health as Warning ({}).", r.status)))
        }
        "Unhealthy" => issues.push((
            Health::Bad,
            format!("Windows reports this disk as Unhealthy ({}). Back it up and replace it.", r.status),
        )),
        _ => issues.push((Health::Unknown, "Windows doesn't know this disk's health.".into())),
    }
    if let Some(w) = r.wear.filter(|w| *w >= 80) {
        issues.push((if w >= 100 { Health::Bad } else { Health::Warn }, format!("{w}% of its rated wear is used.")));
    }
    let errs = r.read_err.unwrap_or(0) + r.write_err.unwrap_or(0);
    if errs > 0 {
        issues.push((Health::Warn, format!("{errs} uncorrected read/write errors.")));
    }
    if let Some(t) = r.temp.filter(|t| *t >= 60) {
        issues.push((if t >= 70 { Health::Bad } else { Health::Warn }, format!("Running hot: {t} °C.")));
    }
    issues.sort_by_key(|i| std::cmp::Reverse(i.0));
    let health = issues.first().map_or(Health::Ok, |i| i.0);
    let rotation_rpm = match r.media.as_str() {
        "SSD" => Some(0),
        "HDD" => r.rpm.filter(|v| *v > 0 && *v < 100_000).map(|v| v as u32),
        _ => None,
    };
    Disk {
        name: format!("PhysicalDisk{}", r.id),
        protocol: if r.bus.is_empty() { "?".into() } else { r.bus },
        model: nonempty(r.model),
        serial: nonempty(r.serial),
        firmware: nonempty(r.firmware),
        capacity: r.size.filter(|s| *s > 0),
        rotation_rpm,
        passed: match r.health.as_str() {
            "Healthy" => Some(true),
            "Unhealthy" => Some(false),
            _ => None,
        },
        temp_c: r.temp.filter(|t| *t > 0).map(|t| t as i32),
        power_on_hours: r.hours.filter(|h| *h > 0),
        health,
        issues: issues.into_iter().map(|i| i.1).collect(),
        attributes: Vec::new(),
        nvme: None,
    }
}

/// Parses the [`DISKS`] JSON.
pub fn parse_disks(json: &str) -> SystemResult<DisksReport> {
    let raw: Vec<RawDisk> = serde_json::from_str(json.trim())
        .map_err(|e| SystemError::other(format!("couldn't read the disk list: {e}")))?;
    let disks: Vec<Disk> = raw.into_iter().map(disk).collect();
    let health = disks.iter().map(|d| d.health).max().unwrap_or(Health::Ok);
    Ok(DisksReport { disks, health })
}

#[derive(Deserialize)]
struct RawPool {
    name: String,
    health: String,
    status: String,
    size: Option<u64>,
    used: Option<u64>,
    #[serde(default)]
    disks: Vec<RawMember>,
    #[serde(default)]
    vdisks: Vec<RawMember>,
    #[serde(default)]
    jobs: Vec<RawJob>,
}

#[derive(Deserialize)]
struct RawMember {
    name: String,
    health: String,
    status: String,
    #[serde(default)]
    layout: Option<String>,
}

#[derive(Deserialize)]
struct RawJob {
    name: String,
    pct: f32,
}

fn word_health(h: &str) -> Health {
    match h {
        "Healthy" => Health::Ok,
        "Warning" => Health::Warn,
        "Unhealthy" => Health::Bad,
        _ => Health::Unknown,
    }
}

/// Parses the [`POOLS`] JSON.
pub fn parse_pools(json: &str) -> SystemResult<PoolsReport> {
    let raw: Vec<RawPool> = serde_json::from_str(json.trim())
        .map_err(|e| SystemError::other(format!("couldn't read the storage pools: {e}")))?;
    let pools: Vec<Pool> = raw
        .into_iter()
        .map(|p| {
            let mut issues: Vec<(Health, String)> = Vec::new();
            let ph = word_health(&p.health);
            if ph != Health::Ok {
                issues.push((ph, format!("The pool is {} ({}).", p.health, p.status)));
            }
            for v in &p.vdisks {
                let h = word_health(&v.health);
                if h != Health::Ok {
                    issues.push((h, format!("Virtual disk {} is {} ({}).", v.name, v.health, v.status)));
                }
            }
            for d in &p.disks {
                let h = word_health(&d.health);
                if h != Health::Ok {
                    issues.push((h, format!("Disk {} is {} ({}).", d.name, d.health, d.status)));
                }
            }
            issues.sort_by_key(|i| std::cmp::Reverse(i.0));
            let layouts: Vec<String> = p
                .vdisks
                .iter()
                .filter_map(|v| v.layout.clone().filter(|l| !l.is_empty()).map(|l| format!("{} ({l})", v.name)))
                .collect();
            let job = p.jobs.first();
            Pool {
                kind: PoolKind::StorageSpaces,
                name: p.name,
                detail: (!layouts.is_empty()).then(|| layouts.join(", ")),
                state: p.status,
                health: issues.first().map_or(ph, |i| i.0.max(ph)),
                size: p.size.filter(|s| *s > 0),
                used: p.used,
                devices: p
                    .disks
                    .into_iter()
                    .map(|d| PoolDevice { name: d.name, state: d.status, errors: None })
                    .collect(),
                issues: issues.into_iter().map(|i| i.1).collect(),
                activity: job.map(|j| format!("{} running", j.name)),
                progress: job.map(|j| j.pct),
            }
        })
        .collect();
    let health = pools.iter().map(|p| p.health).max().unwrap_or(Health::Ok);
    Ok(PoolsReport { pools, health })
}

async fn json(conn: &Connection, script: &str, what: &str) -> SystemResult<String> {
    let o = run(conn, script).await.map_err(SystemError::other)?;
    if o.code != 0 {
        let t = o.error_text();
        return Err(SystemError::other(if t.is_empty() {
            format!("reading {what} failed (exit {})", o.code)
        } else {
            t
        }));
    }
    Ok(o.stdout)
}

pub async fn disks(conn: &Connection) -> SystemResult<DisksReport> {
    parse_disks(&json(conn, DISKS, "the disks").await?)
}

pub async fn pools(conn: &Connection) -> SystemResult<PoolsReport> {
    parse_pools(&json(conn, POOLS, "Storage Spaces").await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disks() {
        let r = parse_disks(
            r#"[{"id":"0","model":"Samsung SSD 980 PRO","serial":"S5GX ","firmware":"5B2QGXA7","bus":"NVMe","media":"SSD","size":1000204886016,"rpm":0,"health":"Healthy","status":"OK","temp":41,"wear":3,"hours":5120,"readErr":0,"writeErr":0},
                {"id":"1","model":"WDC WD80EFZX","serial":"","firmware":"","bus":"SATA","media":"HDD","size":8001563222016,"rpm":5400,"health":"Warning","status":"Predictive Failure","temp":null,"wear":null,"hours":null,"readErr":null,"writeErr":null}]"#,
        )
        .unwrap();
        assert_eq!(r.disks.len(), 2);
        let a = &r.disks[0];
        assert_eq!(
            (a.name.as_str(), a.protocol.as_str(), a.rotation_rpm, a.temp_c, a.health),
            ("PhysicalDisk0", "NVMe", Some(0), Some(41), Health::Ok)
        );
        assert_eq!(a.serial.as_deref(), Some("S5GX"));
        let b = &r.disks[1];
        assert_eq!((b.health, b.rotation_rpm, b.serial.as_deref()), (Health::Warn, Some(5400), None));
        assert!(b.issues[0].contains("Predictive Failure"));
        assert_eq!(r.health, Health::Warn);
        assert_eq!(parse_disks("[]").unwrap().disks.len(), 0);
    }

    #[test]
    fn pools() {
        let r = parse_pools(
            r#"[{"name":"Tank","health":"Warning","status":"Degraded","size":16000000000000,"used":9000000000000,
                 "disks":[{"name":"Disk A","health":"Healthy","status":"OK"},{"name":"Disk B","health":"Unhealthy","status":"Lost Communication"}],
                 "vdisks":[{"name":"Data","health":"Warning","status":"Degraded","layout":"Mirror"}],
                 "jobs":[{"name":"Repair","pct":42}]}]"#,
        )
        .unwrap();
        let p = &r.pools[0];
        assert_eq!(
            (p.kind, p.health, p.detail.as_deref()),
            (PoolKind::StorageSpaces, Health::Bad, Some("Data (Mirror)"))
        );
        assert_eq!(p.devices.len(), 2);
        assert!(p.issues[0].contains("Disk B"));
        assert_eq!(p.progress, Some(42.0));
    }

    #[cfg(windows)]
    #[test]
    fn reads_local_disks() {
        let o = super::super::run_local(DISKS, b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        let r = parse_disks(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", o.stdout));
        assert!(!r.disks.is_empty(), "{}", o.stdout);
        let o = super::super::run_local(POOLS, b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        parse_pools(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", o.stdout));
    }
}

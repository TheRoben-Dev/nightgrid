//! "What's growing" on Windows hosts (D-200): the same report as
//! [`crate::system::growth`], from .NET's directory walk instead of `du`.
//! Junctions and other reparse points are skipped (as `du -x` stays on one
//! filesystem); the previous scan is kept on the host in
//! `%USERPROFILE%\.nightgrid\growth`.

use crate::ssh::client::Connection;
use crate::system::growth::{check_root, parse, GrowthReport, BIG_KIB, KEEP};
use crate::system::{SystemError, SystemResult};

use super::{quote, run};

pub fn script(root: &str) -> String {
    format!(
        r#"$ErrorActionPreference = 'Continue'
$m = {root}
$now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
"@@now $now"
if (-not [IO.Directory]::Exists($m)) {{ '@@missing'; return }}
$G = Join-Path $env:USERPROFILE '.nightgrid\growth'
$sha = [Security.Cryptography.SHA1]::Create()
$k = -join ($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($m)) | ForEach-Object {{ $_.ToString('x2') }})
$f = Join-Path $G "$k.tsv"
if ([IO.File]::Exists($f)) {{ '@@prev'; [IO.File]::ReadAllLines($f) }}
$root = (New-Object IO.DirectoryInfo $m).FullName.TrimEnd('\')
$big = [int64]{BIG_KIB} * 1024
$sizes = @{{}}
$files = New-Object Collections.Generic.List[object]
$denied = 0
$total = [int64]0
$cut = (Get-Date).AddDays(-1)
$stack = New-Object Collections.Generic.Stack[object]
$stack.Push((New-Object IO.DirectoryInfo $root))
while ($stack.Count -gt 0) {{
  $d = $stack.Pop()
  $rel = ''
  if ($d.FullName.Length -gt $root.Length) {{ $rel = $d.FullName.Substring($root.Length + 1).Replace('\', '/') }}
  $parts = @()
  if ($rel) {{ $parts = $rel.Split('/') }}
  try {{ $items = $d.GetFileSystemInfos() }} catch {{ $denied++; continue }}
  foreach ($i in $items) {{
    if ($i.Attributes -band [IO.FileAttributes]::ReparsePoint) {{ continue }}
    $r = $i.Name
    if ($rel) {{ $r = "$rel/$($i.Name)" }}
    if ($i -is [IO.DirectoryInfo]) {{
      $stack.Push($i)
      if ($parts.Count -lt 2) {{ $sizes[$r] = [int64]0 }}
      continue
    }}
    $len = [int64]$i.Length
    $total += $len
    if ($parts.Count -ge 1) {{ $sizes[$parts[0]] += $len }}
    if ($parts.Count -ge 2) {{ $sizes["$($parts[0])/$($parts[1])"] += $len }}
    if ($len -ge $big) {{
      $recent = 0
      if ($i.LastWriteTime -gt $cut) {{ $recent = 1 }}
      $files.Add(@($len, $r, $recent))
    }}
  }}
}}
$rows = New-Object Collections.Generic.List[object]
$rows.Add(@($total, 'd', 0, '.'))
foreach ($e in $sizes.GetEnumerator()) {{ $rows.Add(@([int64]$e.Value, 'd', 0, "./$($e.Key)")) }}
foreach ($x in $files) {{ $rows.Add(@($x[0], 'f', $x[2], "./$($x[1])")) }}
$top = @($rows | Sort-Object {{ $_[0] }} -Descending | Select-Object -First {KEEP})
'@@scan'
$save = New-Object Collections.Generic.List[string]
$save.Add("@at $now")
foreach ($x in $top) {{
  $kib = [int64][math]::Floor($x[0] / 1024)
  "$kib`t$($x[1])`t$($x[2])`t$($x[3])"
  $save.Add("$kib`t$($x[3])")
}}
"@@denied $denied"
try {{
  [void][IO.Directory]::CreateDirectory($G)
  [IO.File]::WriteAllLines($f, $save.ToArray())
}} catch {{ }}
"#,
        root = quote(root),
    )
}

pub async fn scan(conn: &Connection, root: &str) -> SystemResult<GrowthReport> {
    check_root(root)?;
    let o = run(conn, &script(root)).await.map_err(SystemError::other)?;
    if o.code != 0 && !o.stdout.contains("@@now") {
        return Err(SystemError::other(o.error_text()));
    }
    Ok(parse(root, &o.stdout))
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn scans_a_local_folder() {
        let base = std::env::temp_dir().join(format!("ng-growth-win-{}", std::process::id()));
        std::fs::create_dir_all(base.join("vms\\deep")).unwrap();
        std::fs::write(base.join("vms\\deep\\x.bin"), vec![1u8; 64 * 1024]).unwrap();
        let root = base.to_str().unwrap().to_owned();
        let o = super::super::run_local(&super::script(&root), b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        let r = crate::system::growth::parse(&root, &o.stdout);
        assert!(!r.missing && r.at > 0, "{}", o.stdout);
        assert!(r.total >= 64 * 1024, "{}", o.stdout);
        assert!(r.entries.iter().any(|e| e.path == "vms/deep" && e.dir && e.size >= 64 * 1024), "{}", o.stdout);
        let again = super::super::run_local(&super::script(&root), b"").unwrap();
        assert!(crate::system::growth::parse(&root, &again.stdout).prev_at.is_some(), "{}", again.stdout);
        let _ = std::fs::remove_dir_all(&base);
    }
}

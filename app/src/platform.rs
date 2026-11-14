//! Per-platform webview workarounds. Runs first thing in `run()`, before any
//! threads exist, because it mutates the process environment.

/// Linux/WebKitGTK: the DMA-BUF renderer causes blank or corrupted windows on
/// the NVIDIA proprietary driver and some VMs/Wayland setups. Disable it when
/// NVIDIA is detected, unless the user already decided via the env var.
///
/// Overrides:
/// - `WEBKIT_DISABLE_DMABUF_RENDERER=1` (or `0`) set by the user wins.
/// - `NIGHTGRID_FORCE_DMABUF=1` keeps DMA-BUF on even with NVIDIA.
/// - `NIGHTGRID_DISABLE_DMABUF=1` turns it off on any GPU.
#[cfg(target_os = "linux")]
pub fn apply_webview_workarounds() {
    use std::env;

    if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some() {
        return;
    }
    let forced_on = env::var_os("NIGHTGRID_FORCE_DMABUF").is_some();
    let forced_off = env::var_os("NIGHTGRID_DISABLE_DMABUF").is_some();
    let nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists();
    if forced_off || (nvidia && !forced_on) {
        // SAFETY: called at startup before any other thread is spawned.
        unsafe { env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

#[cfg(not(target_os = "linux"))]
pub fn apply_webview_workarounds() {}

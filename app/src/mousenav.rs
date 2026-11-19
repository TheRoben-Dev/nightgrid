//! Mouse side buttons (D-186). WKWebView on macOS doesn't hand buttons 3
//! and 4 to the page, so a local AppKit event monitor catches them on the
//! main window, swallows them and tells the UI (`nav://mouse`). Windows and
//! Linux webviews deliver them to the page, which handles them itself.

#[cfg(target_os = "macos")]
pub fn install(app: &tauri::AppHandle) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};
    use tauri::{Emitter, Manager};

    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("mouse side buttons: not on the main thread");
        return;
    };
    let Some(window) = app.get_webview_window("main") else { return };
    let Ok(ns_window) = window.ns_window() else { return };
    let main = ns_window as usize;
    let app = app.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit hands the monitor a valid event for the call's duration.
        let ev = unsafe { event.as_ref() };
        let dir = match ev.buttonNumber() {
            3 => "back",
            4 => "forward",
            _ => return event.as_ptr(),
        };
        // Web UI windows (D-147) keep their own buttons.
        if !ev.window(mtm).is_some_and(|w| Retained::as_ptr(&w) as usize == main) {
            return event.as_ptr();
        }
        if ev.r#type() == NSEventType::OtherMouseUp {
            let _ = app.emit_to("main", "nav://mouse", dir);
        }
        std::ptr::null_mut()
    });
    // SAFETY: the block has the handler signature AppKit expects; AppKit
    // copies it, and the monitor lives as long as the app.
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::OtherMouseDown | NSEventMask::OtherMouseUp,
            &block,
        )
    };
    match monitor {
        Some(m) => std::mem::forget(m),
        None => tracing::warn!("mouse side buttons: event monitor not installed"),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn install(_app: &tauri::AppHandle) {}

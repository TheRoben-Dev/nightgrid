//! Tauri shell for NIGHTGRID. Commands here are thin adapters over
//! `nightgrid-core`; anything with real logic belongs in the core crate.

mod alerts_cmd;
mod automation_cmd;
mod backup_cmd;
mod backup_jobs_cmd;
mod commands;
mod docker_cmd;
mod files_cmd;
mod helper_cmd;
mod hosts_cmd;
mod integrations_cmd;
mod logging;
mod metrics_cmd;
mod notes_cmd;
mod platform;
mod services_cmd;
mod state;
mod system_cmd;
mod term_cmd;
mod tray;
mod tunnels_cmd;
mod update_cmd;

use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

/// Path the TypeScript bindings are written to (relative to this crate).
pub const BINDINGS_PATH: &str = "../ui/src/lib/bindings.ts";

/// The typed command surface shared by the app and the bindings exporter.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::app_info,
        commands::settings_get,
        commands::settings_set,
        hosts_cmd::hosts_list,
        hosts_cmd::host_create,
        hosts_cmd::host_update,
        hosts_cmd::host_delete,
        hosts_cmd::host_forget_secrets,
        hosts_cmd::groups_list,
        hosts_cmd::group_create,
        hosts_cmd::group_update,
        hosts_cmd::group_delete,
        hosts_cmd::ssh_config_scan,
        hosts_cmd::ssh_config_import,
        hosts_cmd::known_hosts_import_system,
        hosts_cmd::hosts_ping,
        term_cmd::term_open_local,
        term_cmd::term_open_ssh,
        term_cmd::term_write,
        term_cmd::term_resize,
        term_cmd::term_reconnect,
        term_cmd::term_close,
        term_cmd::ssh_trust_host_key,
        term_cmd::ssh_provide_secret,
        files_cmd::fs_home,
        files_cmd::fs_list,
        files_cmd::fs_mkdir,
        files_cmd::fs_rename,
        files_cmd::fs_delete,
        files_cmd::fs_read_text,
        files_cmd::fs_save_text,
        files_cmd::bookmarks_list,
        files_cmd::bookmark_add,
        files_cmd::bookmark_delete,
        files_cmd::transfers_subscribe,
        files_cmd::transfers_add,
        files_cmd::transfer_pause,
        files_cmd::transfer_resume,
        files_cmd::transfer_cancel,
        files_cmd::transfer_resolve,
        files_cmd::transfers_clear,
        docker_cmd::docker_info,
        docker_cmd::docker_containers,
        docker_cmd::docker_stats,
        docker_cmd::docker_action,
        docker_cmd::docker_inspect,
        docker_cmd::docker_logs_start,
        docker_cmd::docker_logs_stop,
        docker_cmd::docker_forget,
        docker_cmd::docker_stacks,
        docker_cmd::docker_compose_run,
        docker_cmd::docker_compose_cancel,
        docker_cmd::docker_snapshots,
        docker_cmd::docker_snapshot_take,
        docker_cmd::docker_snapshot_delete,
        docker_cmd::docker_stack_rollback,
        docker_cmd::docker_stack_check_updates,
        docker_cmd::docker_image_lookup,
        docker_cmd::docker_proxy_routes,
        docker_cmd::docker_stack_deploy,
        docker_cmd::stack_dirs_list,
        docker_cmd::stack_dirs_add,
        docker_cmd::stack_dirs_delete,
        metrics_cmd::metrics_subscribe,
        metrics_cmd::metrics_sync,
        metrics_cmd::metrics_set_visible,
        metrics_cmd::metrics_retry,
        metrics_cmd::metrics_history,
        term_cmd::term_open_docker_exec,
        services_cmd::services_subscribe,
        services_cmd::services_check_all,
        services_cmd::services_list,
        services_cmd::service_create,
        services_cmd::service_update,
        services_cmd::service_delete,
        services_cmd::service_check,
        services_cmd::service_history,
        services_cmd::homepage_detect,
        services_cmd::homepage_preview,
        services_cmd::homepage_import,
        tunnels_cmd::tunnels_subscribe,
        tunnels_cmd::tunnels_list,
        tunnels_cmd::tunnel_create,
        tunnels_cmd::tunnel_update,
        tunnels_cmd::tunnel_delete,
        tunnels_cmd::tunnel_start,
        tunnels_cmd::tunnel_stop,
        tunnels_cmd::tunnel_open_temporary,
        system_cmd::system_info,
        system_cmd::system_sudo_forget,
        system_cmd::system_units,
        system_cmd::system_schedules,
        system_cmd::system_history_search,
        system_cmd::system_security,
        system_cmd::system_unit_status,
        system_cmd::system_unit_action,
        system_cmd::system_journal_start,
        system_cmd::system_stream_stop,
        system_cmd::system_updates,
        system_cmd::system_upgrade_run,
        system_cmd::system_reboot,
        system_cmd::system_disks,
        integrations_cmd::integrations_list,
        integrations_cmd::integration_create,
        integrations_cmd::integration_update,
        integrations_cmd::integration_delete,
        integrations_cmd::integration_reveal_secret,
        integrations_cmd::integration_status,
        integrations_cmd::integration_test,
        integrations_cmd::amp_instance_action,
        notes_cmd::notes_list,
        notes_cmd::note_save,
        notes_cmd::note_delete,
        automation_cmd::snippets_list,
        automation_cmd::snippet_save,
        automation_cmd::snippet_delete,
        automation_cmd::playbooks_list,
        automation_cmd::playbook_save,
        automation_cmd::playbook_delete,
        automation_cmd::automation_inspect,
        automation_cmd::automation_run,
        automation_cmd::automation_stop,
        backup_cmd::backup_export,
        backup_cmd::backup_open,
        backup_cmd::backup_import,
        backup_cmd::backup_pick_folder,
        backup_cmd::backup_folder,
        backup_cmd::backup_clear_folder,
        backup_cmd::app_restart,
        helper_cmd::helper_status,
        helper_cmd::helper_download,
        helper_cmd::helper_cancel_download,
        helper_cmd::helper_delete_model,
        helper_cmd::helper_unload,
        helper_cmd::helper_ask,
        helper_cmd::helper_stop,
        alerts_cmd::alerts_subscribe,
        alerts_cmd::alerts_refresh,
        alerts_cmd::alerts_clear_history,
        alerts_cmd::alerts_webhook_get,
        alerts_cmd::alerts_webhook_set,
        alerts_cmd::alerts_test,
        alerts_cmd::alerts_log_samples,
        backup_jobs_cmd::backup_jobs_list,
        backup_jobs_cmd::backup_job_save,
        backup_jobs_cmd::backup_job_delete,
        backup_jobs_cmd::backup_jobs_check,
        backup_jobs_cmd::backup_jobs_detect,
        update_cmd::update_check,
        update_cmd::update_install,
    ])
}

/// Writes `ui/src/lib/bindings.ts`.
pub fn export_bindings(builder: &Builder<tauri::Wry>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS_PATH);
    builder
        .export(
            specta_typescript::Typescript::default()
                .header("// Generated by tauri-specta from app/src/lib.rs. Do not edit.\n// Regenerate: cargo test -p nightgrid --lib bindings\n"),
            &path,
        )
        .expect("failed to export TypeScript bindings");
}

pub fn run() {
    platform::apply_webview_workarounds();

    let builder = specta_builder();
    #[cfg(debug_assertions)]
    export_bindings(&builder);

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().pubkey(update_cmd::PUBKEY.trim()).build())
        .manage(update_cmd::PendingUpdate::default())
        .manage(helper_cmd::Helper::default())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let log_dir = app.path().app_log_dir()?;
            let guard = logging::init(&log_dir)?;
            app.manage(guard);

            let data_dir = app.path().app_data_dir()?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), ?data_dir, "starting NIGHTGRID");
            let state = state::AppState::open(&data_dir, log_dir)?;
            app.manage(state);
            tray::apply(app.handle());
            alerts_cmd::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close to tray: hide instead of quitting while a tray icon exists.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let to_tray = app.try_state::<state::AppState>().is_some_and(|s| {
                    nightgrid_core::settings::AppSettings::load(&s.store()).map(|c| c.close_to_tray).unwrap_or(false)
                });
                if to_tray && tray::exists(app) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building NIGHTGRID")
        .run(|app, event| {
            // macOS: clicking the Dock icon brings a hidden window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                tray::show_main(app);
            }
            let _ = (app, event);
        });
}

#[cfg(test)]
mod tests {
    /// Regenerates the TS bindings. CI runs this and fails if the committed
    /// file differs, so the frontend and backend can't drift apart.
    #[test]
    fn bindings() {
        super::export_bindings(&super::specta_builder());
    }
}

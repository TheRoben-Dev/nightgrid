//! Tauri shell for NIGHTGRID. Commands here are thin adapters over
//! `nightgrid-core`; anything with real logic belongs in the core crate.

mod access_cmd;
mod alerts_cmd;
mod automation_cmd;
mod backup_cmd;
mod backup_jobs_cmd;
mod blueprint_cmd;
mod capacity_cmd;
mod catalog_cmd;
mod chores;
mod cmdwidget_cmd;
mod commands;
mod confdiff_cmd;
mod desktop_cmd;
mod docker_cmd;
mod drift_cmd;
mod drills_cmd;
mod envvault_cmd;
mod expiry_cmd;
mod files_cmd;
mod firewall_cmd;
mod gitstack_cmd;
mod helper_cmd;
mod hosts_cmd;
mod integrations_cmd;
mod jobs_cmd;
mod journal_cmd;
mod kbpack_cmd;
mod keys_cmd;
mod logging;
mod logsearch_cmd;
mod mcp_cmd;
mod metrics_cmd;
mod netperf_cmd;
mod netwatch_cmd;
mod node_cmd;
mod notes_cmd;
mod notify_cmd;
mod platform;
mod posture_cmd;
mod powercost_cmd;
mod proxmox_cmd;
mod proxy_cmd;
mod quest_cmd;
mod replay_cmd;
mod secrets_cmd;
mod services_cmd;
mod state;
mod status_cmd;
mod system_cmd;
mod tailnet_cmd;
mod term_cmd;
mod timeline_cmd;
mod tray;
mod tunnels_cmd;
mod update_cmd;
mod ups_cmd;
mod uptime_cmd;
mod webui_cmd;

use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

/// Path the TypeScript bindings are written to (relative to this crate).
pub const BINDINGS_PATH: &str = "../ui/src/lib/bindings.ts";

/// Commands outside the typed surface (raw byte responses).
fn raw_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![desktop_cmd::desktop_frame]
}

/// The typed command surface shared by the app and the bindings exporter.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::app_info,
        commands::diagnostics_export,
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
        term_cmd::ssh_auth_notices,
        term_cmd::term_write,
        term_cmd::term_resize,
        term_cmd::term_foreground,
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
        webui_cmd::webui_open,
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
        system_cmd::system_power,
        system_cmd::host_wake,
        system_cmd::power_schedules,
        system_cmd::power_schedule_set,
        system_cmd::power_schedule_remove,
        keys_cmd::keys_list,
        keys_cmd::keys_generate,
        keys_cmd::keys_authorized,
        keys_cmd::keys_push,
        keys_cmd::keys_remove,
        keys_cmd::keys_verify,
        timeline_cmd::timeline_get,
        timeline_cmd::timeline_clear,
        journal_cmd::journal_query,
        journal_cmd::journal_stats,
        journal_cmd::journal_note,
        journal_cmd::journal_edit,
        journal_cmd::journal_delete,
        journal_cmd::journal_clear,
        journal_cmd::journal_for_drift,
        journal_cmd::journal_export,
        blueprint_cmd::blueprint_capture,
        blueprint_cmd::blueprint_save,
        blueprint_cmd::blueprint_open,
        blueprint_cmd::blueprint_close,
        blueprint_cmd::blueprint_plan,
        blueprint_cmd::blueprint_apply,
        expiry_cmd::watch_list,
        expiry_cmd::watch_save,
        expiry_cmd::watch_delete,
        expiry_cmd::watch_check,
        expiry_cmd::watch_lookup,
        confdiff_cmd::confdiff_baseline,
        confdiff_cmd::confdiff_save_baseline,
        confdiff_cmd::confdiff_compare,
        confdiff_cmd::confdiff_file,
        system_cmd::system_disks,
        system_cmd::system_pools,
        system_cmd::disk_forecasts,
        integrations_cmd::integrations_list,
        integrations_cmd::integration_create,
        integrations_cmd::integration_update,
        integrations_cmd::integration_delete,
        integrations_cmd::integration_reveal_secret,
        integrations_cmd::integration_status,
        integrations_cmd::integration_test,
        integrations_cmd::amp_instance_action,
        integrations_cmd::dns_filter_action,
        envvault_cmd::env_vars_list,
        envvault_cmd::env_stacks,
        envvault_cmd::env_var_set,
        envvault_cmd::env_var_delete,
        envvault_cmd::env_var_reveal,
        envvault_cmd::env_vars_order,
        envvault_cmd::env_import_host,
        envvault_cmd::env_deploy_preview,
        envvault_cmd::env_deploy,
        gitstack_cmd::git_stacks_list,
        gitstack_cmd::git_stack_add,
        gitstack_cmd::git_stack_check,
        gitstack_cmd::git_stack_diff,
        gitstack_cmd::git_stack_deploy,
        gitstack_cmd::git_stack_remove,
        netperf_cmd::net_test_run,
        netperf_cmd::net_tests_list,
        netperf_cmd::net_tests_clear,
        cmdwidget_cmd::cmd_widgets_list,
        cmdwidget_cmd::cmd_widget_save,
        cmdwidget_cmd::cmd_widget_delete,
        cmdwidget_cmd::cmd_widget_run,
        cmdwidget_cmd::cmd_widget_try,
        proxmox_cmd::pve_overview,
        proxmox_cmd::pve_guest_action,
        proxmox_cmd::pve_task,
        proxmox_cmd::pve_snapshots,
        proxmox_cmd::pve_snapshot_create,
        proxmox_cmd::pve_snapshot_rollback,
        proxmox_cmd::pve_snapshot_delete,
        proxmox_cmd::pve_backups,
        proxmox_cmd::pve_backup_now,
        proxmox_cmd::pve_restore,
        proxmox_cmd::pve_backup_protect,
        proxmox_cmd::pve_backup_delete,
        proxmox_cmd::pve_create_info,
        proxmox_cmd::pve_vm_create,
        proxmox_cmd::pve_ct_create,
        proxmox_cmd::pve_clone,
        proxmox_cmd::pve_template,
        proxmox_cmd::pve_destroy,
        proxmox_cmd::pve_iso_download,
        proxmox_cmd::pve_appliances,
        proxmox_cmd::pve_appliance_download,
        proxmox_cmd::pve_config,
        proxmox_cmd::pve_config_set,
        proxmox_cmd::pve_disk_resize,
        proxmox_cmd::pve_host_devices,
        proxmox_cmd::pve_network,
        proxmox_cmd::pve_iface_save,
        proxmox_cmd::pve_iface_delete,
        proxmox_cmd::pve_network_revert,
        proxmox_cmd::pve_network_apply,
        proxmox_cmd::pve_network_settle,
        proxmox_cmd::pve_firewall,
        proxmox_cmd::pve_fw_rule_save,
        proxmox_cmd::pve_fw_rule_move,
        proxmox_cmd::pve_fw_rule_delete,
        proxmox_cmd::pve_fw_options,
        proxmox_cmd::pve_fw_object,
        proxmox_cmd::pve_fw_settle,
        proxmox_cmd::pve_sandbox_create,
        proxmox_cmd::pve_sandbox_isolate,
        proxmox_cmd::pve_sandbox_extend,
        proxmox_cmd::pve_sandbox_keep,
        proxmox_cmd::pve_sandbox_destroy,
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
        helper_cmd::helper_models,
        helper_cmd::helper_download,
        helper_cmd::helper_cancel_download,
        helper_cmd::helper_delete_model,
        helper_cmd::helper_unload,
        helper_cmd::helper_ask,
        helper_cmd::helper_stop,
        helper_cmd::helper_host_history,
        drift_cmd::helper_host_lookups,
        alerts_cmd::alerts_subscribe,
        alerts_cmd::alerts_refresh,
        alerts_cmd::alerts_clear_history,
        alerts_cmd::alerts_deps_list,
        alerts_cmd::alerts_deps_add,
        alerts_cmd::alerts_deps_delete,
        ups_cmd::ups_list,
        powercost_cmd::power_overview,
        quest_cmd::quest_check,
        node_cmd::node_list,
        node_cmd::node_check,
        node_cmd::node_sync_now,
        node_cmd::node_set_use,
        node_cmd::node_release,
        node_cmd::node_install,
        node_cmd::node_uninstall,
        powercost_cmd::power_profile_set,
        powercost_cmd::power_profile_clear,
        ups_cmd::ups_save,
        ups_cmd::ups_delete,
        ups_cmd::ups_status,
        ups_cmd::ups_read_now,
        ups_cmd::ups_cancel,
        uptime_cmd::uptime_days,
        uptime_cmd::uptime_outages,
        uptime_cmd::uptime_report,
        uptime_cmd::uptime_report_export,
        access_cmd::access_audit,
        jobs_cmd::cron_read,
        jobs_cmd::cron_apply,
        jobs_cmd::cron_describe,
        jobs_cmd::timer_jobs,
        jobs_cmd::timer_job_save,
        jobs_cmd::timer_job_remove,
        jobs_cmd::timer_calendar_preview,
        firewall_cmd::firewall_read,
        firewall_cmd::firewall_check,
        firewall_cmd::firewall_apply,
        firewall_cmd::firewall_test,
        firewall_cmd::firewall_keep,
        firewall_cmd::firewall_undo,
        proxy_cmd::proxy_edit_targets,
        proxy_cmd::caddy_edit_preview,
        proxy_cmd::caddy_save,
        proxy_cmd::traefik_save,
        logsearch_cmd::log_search,
        logsearch_cmd::log_searches_list,
        logsearch_cmd::log_search_save,
        logsearch_cmd::log_search_delete,
        alerts_cmd::alerts_webhook_get,
        alerts_cmd::alerts_webhook_set,
        alerts_cmd::alerts_test,
        notify_cmd::notices_list,
        notify_cmd::notices_read,
        notify_cmd::notices_clear,
        notify_cmd::ntfy_get,
        notify_cmd::ntfy_set,
        notify_cmd::notify_test,
        posture_cmd::posture_list,
        posture_cmd::posture_history,
        posture_cmd::posture_check,
        secrets_cmd::secrets_list,
        secrets_cmd::secret_mark_rotated,
        secrets_cmd::secret_policy,
        secrets_cmd::secret_manual_save,
        secrets_cmd::secret_manual_delete,
        secrets_cmd::secret_key_hosts,
        capacity_cmd::capacity_list,
        catalog_cmd::catalog_build,
        replay_cmd::replay_incidents,
        replay_cmd::replay_load,
        catalog_cmd::catalog_markdown,
        catalog_cmd::report_save,
        drills_cmd::drills_list,
        drills_cmd::drill_save,
        drills_cmd::drill_delete,
        drills_cmd::drill_runs,
        drills_cmd::drill_run,
        drills_cmd::drill_report,
        alerts_cmd::alerts_log_samples,
        alerts_cmd::alerts_log_mute,
        alerts_cmd::log_classify,
        backup_jobs_cmd::backup_jobs_list,
        backup_jobs_cmd::device_labels_list,
        tailnet_cmd::tailnet_status,
        tailnet_cmd::tailnet_set_enabled,
        tailnet_cmd::tailnet_configure,
        tailnet_cmd::tailnet_login,
        tailnet_cmd::tailnet_logout,
        backup_jobs_cmd::device_label_save,
        backup_jobs_cmd::device_label_delete,
        backup_jobs_cmd::backup_job_save,
        backup_jobs_cmd::backup_job_delete,
        backup_jobs_cmd::backup_jobs_check,
        backup_jobs_cmd::backup_jobs_detect,
        backup_jobs_cmd::backup_tests_list,
        backup_jobs_cmd::backup_test_restore,
        system_cmd::system_vms,
        system_cmd::netmap_discover,
        system_cmd::netmap_discover_local,
        system_cmd::netmap_inventory,
        system_cmd::netmap_device_trust,
        system_cmd::netmap_device_forget,
        system_cmd::netmap_layout_get,
        system_cmd::netmap_layout_save,
        system_cmd::netmap_networks,
        system_cmd::netmap_network_scan,
        system_cmd::netmap_network_rename,
        system_cmd::netmap_network_forget,
        system_cmd::netmap_network_merge,
        system_cmd::net_tool,
        system_cmd::netmap_security,
        netwatch_cmd::netmap_watch_last,
        system_cmd::net_mac_vendor,
        system_cmd::net_wake,
        system_cmd::system_vm_action,
        desktop_cmd::desktop_open,
        desktop_cmd::desktop_input,
        desktop_cmd::desktop_close,
        desktop_cmd::desktop_guest_addrs,
        desktop_cmd::desktop_rdp_open,
        desktop_cmd::desktop_rdp_list,
        desktop_cmd::desktop_rdp_close,
        term_cmd::term_open_vm_console,
        status_cmd::status_page_get,
        status_cmd::status_page_set,
        status_cmd::status_page_new_token,
        status_cmd::dashboard_export,
        mcp_cmd::mcp_status,
        mcp_cmd::mcp_set,
        mcp_cmd::mcp_new_token,
        mcp_cmd::mcp_answer,
        update_cmd::update_check,
        update_cmd::update_install,
        kbpack_cmd::kb_pack_status,
        kbpack_cmd::kb_pack_check,
        kbpack_cmd::kb_pack_load,
        kbpack_cmd::kb_pack_remove,
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

/// `nightgrid --mcp`: an MCP stdio server that forwards to the running
/// app. Returns the process exit code.
pub fn mcp_stdio() -> i32 {
    mcp_cmd::stdio_bridge()
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
        .manage(kbpack_cmd::KbPacks::default())
        .manage(helper_cmd::Helper::default())
        .manage(desktop_cmd::Desktops::default())
        .manage(webui_cmd::WebWindows::default())
        .invoke_handler({
            // desktop_frame returns raw bytes, which specta can't type, so it's
            // registered beside the typed commands.
            let typed = builder.invoke_handler();
            let raw = raw_handler();
            move |invoke| {
                // Only the app's own window talks to the backend. Web UI
                // windows show remote pages and get no IPC at all (D-147).
                if invoke.message.webview().label() != "main" {
                    invoke.resolver.reject("not allowed from this window");
                    return true;
                }
                if invoke.message.command() == "desktop_frame" {
                    raw(invoke)
                } else {
                    typed(invoke)
                }
            }
        })
        .setup(move |app| {
            builder.mount_events(app);

            let log_dir = app.path().app_log_dir()?;
            let guard = logging::init(&log_dir)?;
            app.manage(guard);

            let data_dir = app.path().app_data_dir()?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), ?data_dir, "starting NIGHTGRID");
            let state = state::AppState::open(&data_dir, log_dir)?;
            if let Ok(hosts) = state.store().list_hosts() {
                hosts_cmd::sync_fallbacks(&hosts);
            }
            app.manage(state);
            tray::apply(app.handle());
            notify_cmd::migrate(app.handle());
            alerts_cmd::start(app.handle());
            status_cmd::apply(app.handle());
            mcp_cmd::apply(app.handle());
            tailnet_cmd::start_if_enabled(app.handle());
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
            // Quit (menu, tray, Cmd+Q): free the helper's model first, or
            // llama.cpp's Metal teardown aborts during exit.
            if let tauri::RunEvent::Exit = event {
                helper_cmd::shutdown(app);
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

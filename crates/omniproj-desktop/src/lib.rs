//! OmniProj desktop backend (R0).
//!
//! This library is the application boundary: typed DTOs, a fixed serialized error
//! contract, plus the focused MVP Record/Advance commands. The pre-R0 command surface is
//! archived verbatim in `legacy.rs`; only the reviewed MVP subset is compiled into the
//! shipped binary.

pub mod commands;
pub mod dto;
pub mod error;
pub mod repository_cache;
pub mod service;
pub mod state;
pub mod mvp;

// NOTE: `legacy.rs` is deliberately not a module. It is a read-only source archive.

use tauri::ipc::Invoke;
use tauri::Runtime;

use crate::service::{DesktopService, R0Service, SystemClock};

/// The exact R0 command allowlist, as a reusable invoke handler. Both `run()` and the
/// behavior-level IPC tests install this same handler, so the shipped boundary is what
/// is tested.
pub fn r0_invoke_handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::list_project_index,
        commands::get_project_overview,
        commands::validate_project_source,
        commands::register_project,
        commands::relink_project_source,
        commands::refresh_projects,
        commands::complete_project_setup,
        commands::save_project_framing,
        commands::set_project_status,
        commands::set_commitment,
        commands::confirm_commitment,
        commands::complete_commitment,
        commands::replace_commitment,
        commands::clear_commitment,
        commands::undo_commitment_transition,
        commands::get_tasks,
        commands::get_attention_summary,
        commands::add_task,
        commands::update_task,
        commands::remove_task,
        commands::attribute_commit,
        commands::unattribute_commit,
        commands::get_commit_timeline,
        commands::advance_task,
        commands::adopt_subtasks,
    ]
}

/// Run the OmniProj desktop application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::TrayIconBuilder;
            use tauri::Manager;
            use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

            let service = match DesktopService::initialize(SystemClock) {
                Ok(service) => service,
                Err(error) => {
                    eprintln!("OmniProj could not open its local store: {error}");
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    let handle = app.handle().clone();
                    app.dialog()
                        .message(format!(
                            "无法打开或迁移 OmniProj 本地数据。\n\n{error}\n\n你的项目仓库未被更改。"
                        ))
                        .title("OmniProj 无法启动")
                        .kind(MessageDialogKind::Error)
                        .show(move |_| handle.exit(1));
                    return Ok(());
                }
            };
            let attention_count = service.list_project_index().map(|r| r.projects.iter().filter(|p| !p.review_reasons.is_empty()).count()).unwrap_or(0);
            app.manage(service);
            let show = MenuItemBuilder::with_id("show", "打开 OmniProj").build(app)?;
            let attention = MenuItemBuilder::with_id("attention", format!("待关注项目：{attention_count}")).enabled(false).build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &attention, &quit]).build()?;
            if attention_count > 0 {
                use tauri_plugin_notification::NotificationExt;
                let _ = app.notification().builder().title("OmniProj 待关注提醒").body(format!("有 {attention_count} 个项目需要关注。打开 OmniProj 查看下一步。")) .show();
            }
            let tray = TrayIconBuilder::with_id("main")
                .icon(
                    app.default_window_icon()
                        .expect("app icon set in tauri.conf.json")
                        .clone(),
                )
                .tooltip("OmniProj")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(tray); // keep the tray alive for the app's lifetime
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .invoke_handler(r0_invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running omniproj desktop");
}

//! OmniProj desktop backend (R0).
//!
//! This library is the application boundary: typed DTOs, a fixed serialized error
//! contract, plus the focused MVP Record/Advance commands. The pre-R0 command surface is
//! archived verbatim in `legacy.rs`; only the reviewed MVP subset is compiled into the
//! shipped binary.

pub mod agent_settings;
pub mod commands;
pub mod dto;
pub mod error;
pub mod mvp;
pub mod repository_cache;
pub mod service;
pub mod state;

// NOTE: `legacy.rs` is deliberately not a module. It is a read-only source archive.

use tauri::ipc::Invoke;
use tauri::Runtime;

use crate::service::{DesktopService, SystemClock};

/// The reviewed desktop command allowlist, as a reusable invoke handler. Both `run()` and the
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
        commands::refresh_attention_indicator,
        commands::add_task,
        commands::update_task,
        commands::remove_task,
        commands::attribute_commit,
        commands::unattribute_commit,
        commands::get_commit_timeline,
        commands::get_git_graph,
        commands::advance_task,
        commands::adopt_subtasks,
        commands::promote_task_to_commitment,
        commands::get_plan,
        commands::add_plan_entry,
        commands::set_plan_status,
        commands::set_plan_commit,
        commands::get_reminder_settings,
        commands::set_reminder_settings,
        commands::test_reminder,
        commands::get_dogfood_summary,
        commands::record_reentry_event,
        commands::get_agent_settings,
        commands::set_agent_settings,
        commands::test_agent_provider,
    ]
}

/// Menu-bar state shared by explicit command-triggered refreshes and the hourly
/// background check. The count is shown as native title text beside the icon on macOS.
pub struct AttentionTrayUi {
    update: std::sync::Arc<dyn Fn(usize) + Send + Sync>,
}

impl Clone for AttentionTrayUi {
    fn clone(&self) -> Self {
        Self {
            update: self.update.clone(),
        }
    }
}

impl AttentionTrayUi {
    pub fn apply(&self, count: usize) {
        (self.update)(count);
    }
}

pub fn sync_attention_ui(ui: &AttentionTrayUi) -> crate::mvp::AttentionSummaryDto {
    let settings = crate::mvp::load_reminder_settings();
    let summary = crate::mvp::attention_summary_with_threshold(settings.silent_days_threshold);
    ui.apply(summary.count);
    summary
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
            let reminder_settings = crate::mvp::load_reminder_settings();
            let attention_summary = crate::mvp::attention_summary_with_threshold(reminder_settings.silent_days_threshold);
            let attention_count = attention_summary.count;
            app.manage(service);
            let show = MenuItemBuilder::with_id("show", "打开 OmniProj").build(app)?;
            let attention = MenuItemBuilder::with_id("attention", format!("待关注项目：{attention_count}")).enabled(false).build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &attention, &quit]).build()?;
            if crate::mvp::claim_daily_reminder(&reminder_settings, &attention_summary).unwrap_or(false) {
                use tauri_plugin_notification::NotificationExt;
                let _ = app.notification().builder().title("OmniProj 待关注提醒").body(format!("有 {attention_count} 个项目需要关注。打开 OmniProj 查看下一步。")) .show();
            }
            let notification_handle = app.handle().clone();
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
            let tray_for_update = tray.clone();
            let attention_for_update = attention.clone();
            let attention_ui = AttentionTrayUi {
                update: std::sync::Arc::new(move |count| {
                    let title = (count > 0).then(|| count.to_string());
                    let _ = tray_for_update.set_title(title.as_deref());
                    let _ = tray_for_update.set_tooltip(Some(format!("OmniProj · 待关注项目：{count}")));
                    let _ = attention_for_update.set_text(format!("待关注项目：{count}"));
                }),
            };
            app.manage(attention_ui.clone());
            let background_attention_ui = attention_ui.clone();
            tauri::async_runtime::spawn(async move {
                use std::time::Duration;
                use tauri_plugin_notification::NotificationExt;
                loop {
                    tokio::time::sleep(Duration::from_secs(3_600)).await;
                    let settings = crate::mvp::load_reminder_settings();
                    let summary = sync_attention_ui(&background_attention_ui);
                    if crate::mvp::claim_daily_reminder(&settings, &summary).unwrap_or(false) {
                        let _ = notification_handle.notification().builder()
                            .title("OmniProj 每日待关注提醒")
                            .body(format!("有 {} 个项目需要关注。打开 OmniProj 查看下一步。", summary.count))
                            .show();
                    }
                }
            });
            app.manage(tray); // keep the tray alive for the app's lifetime
            sync_attention_ui(&attention_ui);
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .invoke_handler(r0_invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running omniproj desktop");
}

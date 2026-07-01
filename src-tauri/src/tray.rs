//! VoiceInput v2 — 系统托盘
//!
//! 使用 Tauri 2 的 TrayIconBuilder / MenuBuilder 创建系统托盘图标。
//! 图标用代码生成 32x32 RGBA（绿色圆形 + 白色麦克风简化图形）。
//! 右键菜单: 显示面板 / 设置 / 分隔 / Auto / CN / EN / 分隔 / 预加载模型 / 释放显存 / 分隔 / 退出
//! 单击托盘图标 → 显示悬浮窗

use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem, MenuEvent, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// 菜单项 ID 常量
const MENU_SHOW_PANEL: &str = "show_panel";
const MENU_SETTINGS: &str = "settings";
const MENU_LANG_AUTO: &str = "lang_auto";
const MENU_LANG_CN: &str = "lang_cn";
const MENU_LANG_EN: &str = "lang_en";
const MENU_LOAD_MODEL: &str = "load_model";
const MENU_UNLOAD_MODEL: &str = "unload_model";
const MENU_VIEW_LOGS: &str = "view_logs";
const MENU_QUIT: &str = "quit";

/// 用代码生成 32x32 RGBA 托盘图标。
///
/// 绿色圆形 (#34C759) + 白色简化麦克风图形。
/// 返回 (width, height, Vec<u8> RGBA 数据)。
fn generate_tray_icon() -> (u32, u32, Vec<u8>) {
    let width = 32u32;
    let height = 32u32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = 14.0f32;
    let green_r = 0x34u8;
    let green_g = 0xC7u8;
    let green_b = 0x59u8;

    // 圆形背景
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * width + x) * 4) as usize;
            if dist <= radius {
                // 圆内填充绿色
                rgba[idx] = green_r;
                rgba[idx + 1] = green_g;
                rgba[idx + 2] = green_b;
                rgba[idx + 3] = 255;
            } else {
                // 圆外透明
                rgba[idx + 3] = 0;
            }
        }
    }

    // 绘制简化麦克风图形（白色）
    // 麦克风胶囊体: 矩形 (13,7) 到 (19,18)，圆角
    let mic_r = 255u8;
    let mic_g = 255u8;
    let mic_b = 255u8;

    // 麦克风主体（圆角矩形）
    for y in 7..=18 {
        for x in 13..=19 {
            // 圆角处理
            let corner_dist = if (x == 13 || x == 19) && (y == 7 || y == 18) {
                let dx = x as f32 - if x == 13 { 14.0 } else { 18.0 };
                let dy = y as f32 - if y == 7 { 8.0 } else { 17.0 };
                (dx * dx + dy * dy).sqrt()
            } else {
                0.0
            };

            if corner_dist <= 1.5 {
                let idx = ((y * width + x) * 4) as usize;
                rgba[idx] = mic_r;
                rgba[idx + 1] = mic_g;
                rgba[idx + 2] = mic_b;
                rgba[idx + 3] = 255;
            }
        }
    }

    // 麦克风支架（弧形底部）
    // 水平线 y=19, x=12..=20
    for x in 12..=20 {
        let y = 19;
        let idx = ((y * width + x) * 4) as usize;
        if rgba[idx + 3] > 0 {
            rgba[idx] = mic_r;
            rgba[idx + 1] = mic_g;
            rgba[idx + 2] = mic_b;
        }
    }
    // 垂直线 x=16, y=20..=23
    for y in 20..=23 {
        let x = 16;
        let idx = ((y * width + x) * 4) as usize;
        if rgba[idx + 3] > 0 {
            rgba[idx] = mic_r;
            rgba[idx + 1] = mic_g;
            rgba[idx + 2] = mic_b;
        }
    }
    // 底座水平线 y=23, x=14..=18
    for x in 14..=18 {
        let y = 23;
        let idx = ((y * width + x) * 4) as usize;
        if rgba[idx + 3] > 0 {
            rgba[idx] = mic_r;
            rgba[idx + 1] = mic_g;
            rgba[idx + 2] = mic_b;
        }
    }

    (width, height, rgba)
}

/// 处理菜单点击事件。
fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    log::info!("托盘菜单点击: {}", id);

    match id {
        MENU_SHOW_PANEL => {
            if let Some(window) = app.get_webview_window("floating") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        MENU_SETTINGS => {
            // VoiceInput 只有一个 floating 窗口，设置面板通过前端视图切换实现。
            // 总是 emit 事件，由前端切换到 settings 视图并显示窗口。
            if let Some(window) = app.get_webview_window("floating") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            let _ = app.emit("open-settings", ());
        }
        MENU_LANG_AUTO => {
            let _ = app.emit("set-language", "auto");
        }
        MENU_LANG_CN => {
            let _ = app.emit("set-language", "Chinese");
        }
        MENU_LANG_EN => {
            let _ = app.emit("set-language", "English");
        }
        MENU_LOAD_MODEL => {
            let _ = app.emit("load-model", ());
        }
        MENU_UNLOAD_MODEL => {
            let _ = app.emit("unload-model", ());
        }
        MENU_QUIT => {
            log::info!("用户点击退出，应用即将关闭");
            app.exit(0);
        }
        MENU_VIEW_LOGS => {
            let log_dir = crate::config::get_config_dir().join("logs");
            let _ = std::fs::create_dir_all(&log_dir);
            log::info!("打开日志目录: {}", log_dir.display());
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("explorer")
                    .arg(log_dir.to_string_lossy().to_string())
                    .spawn();
            }
        }
        _ => {}
    }
}

/// 创建系统托盘图标。
///
/// # 参数
/// - `app`: Tauri AppHandle
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    // 生成图标
    let (width, height, rgba) = generate_tray_icon();
    let icon = tauri::image::Image::new(&rgba, width, height);

    // 构建菜单
    let show_panel = MenuItem::with_id(app, MENU_SHOW_PANEL, "显示面板", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, MENU_SETTINGS, "设置", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let lang_auto = MenuItem::with_id(app, MENU_LANG_AUTO, "Auto", true, None::<&str>)?;
    let lang_cn = MenuItem::with_id(app, MENU_LANG_CN, "CN", true, None::<&str>)?;
    let lang_en = MenuItem::with_id(app, MENU_LANG_EN, "EN", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let load_model = MenuItem::with_id(app, MENU_LOAD_MODEL, "预加载模型", true, None::<&str>)?;
    let unload_model = MenuItem::with_id(app, MENU_UNLOAD_MODEL, "释放显存", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let view_logs = MenuItem::with_id(app, MENU_VIEW_LOGS, "查看日志", true, None::<&str>)?;
    let sep4 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_panel,
            &settings,
            &sep1,
            &lang_auto,
            &lang_cn,
            &lang_en,
            &sep2,
            &load_model,
            &unload_model,
            &sep3,
            &view_logs,
            &sep4,
            &quit,
        ],
    )?;

    // 构建托盘图标
    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("VoiceInput - 语音输入法")
        .menu(&menu)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            // 单击托盘图标 → 显示悬浮窗
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("floating") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    log::info!("系统托盘已创建");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_icon_dimensions() {
        let (w, h, rgba) = generate_tray_icon();
        assert_eq!(w, 32);
        assert_eq!(h, 32);
        assert_eq!(rgba.len(), 32 * 32 * 4);
    }

    #[test]
    fn test_generate_icon_has_green_center() {
        let (_w, _h, rgba) = generate_tray_icon();
        // 检查圆内但麦克风图形外的像素是否为绿色
        // (10, 16) 位于圆内（距中心 6 < 半径 14），且不在麦克风图形区域内
        let cx = 10;
        let cy = 16;
        let idx = ((cy * 32 + cx) * 4) as usize;
        assert_eq!(rgba[idx], 0x34); // R
        assert_eq!(rgba[idx + 1], 0xC7); // G
        assert_eq!(rgba[idx + 2], 0x59); // B
        assert_eq!(rgba[idx + 3], 255); // A
    }
}

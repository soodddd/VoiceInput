//! Focus-safe text injection for Windows.
//!
//! Text is injected with Unicode `SendInput`, so image/file clipboard data is
//! never destroyed and delayed clipboard restoration cannot overwrite a newer
//! user copy operation.

#![cfg(windows)]

use std::sync::atomic::{AtomicIsize, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsWindow, SetForegroundWindow,
};

static LAST_EXTERNAL_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// Continuously remember the most recently focused window outside VoiceInput.
/// This makes clicking the floating microphone paste back into the editor the
/// user was using before VoiceInput gained focus.
pub fn start_focus_tracker(app: AppHandle) {
    thread::spawn(move || {
        let own_window = app
            .get_webview_window("floating")
            .and_then(|window| window.hwnd().ok())
            .map(|handle| handle.0 as isize)
            .unwrap_or_default();
        loop {
            let foreground = unsafe { GetForegroundWindow() };
            let raw = foreground.0 as isize;
            if raw != 0 && raw != own_window {
                LAST_EXTERNAL_WINDOW.store(raw, Ordering::Release);
            }
            thread::sleep(Duration::from_millis(150));
        }
    });
}

/// Paste text into the last external foreground window.
///
/// The legacy clipboard options remain in the signature for configuration
/// compatibility. They are intentionally unused because direct Unicode input
/// is safer and leaves every clipboard format untouched.
pub fn paste_text(
    text: String,
    delay_ms: u64,
) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    let target = HWND(LAST_EXTERNAL_WINDOW.load(Ordering::Acquire));
    if target.0 == 0 || !unsafe { IsWindow(target).as_bool() } {
        return Err("未找到原输入窗口；识别结果已保留，可点击“复制”".to_string());
    }
    unsafe {
        let _ = SetForegroundWindow(target);
    }

    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms.min(3000)));
    }
    if unsafe { GetForegroundWindow() } != target {
        return Err("无法切回原输入窗口；识别结果已保留，可点击“复制”".to_string());
    }
    send_unicode_text(&text)?;
    log::info!("已输入识别结果（{} 字符）", text.chars().count());
    Ok(())
}

fn send_unicode_text(text: &str) -> Result<(), String> {
    let mut batch = Vec::with_capacity(512);
    for unit in text.encode_utf16() {
        if unit == b'\n' as u16 || unit == b'\r' as u16 {
            // Treat CRLF as one Enter key.
            if unit == b'\r' as u16 {
                continue;
            }
            batch.push(make_virtual_key(VK_RETURN.0, false));
            batch.push(make_virtual_key(VK_RETURN.0, true));
        } else {
            batch.push(make_unicode_input(unit, false));
            batch.push(make_unicode_input(unit, true));
        }

        if batch.len() >= 500 {
            send_batch(&batch)?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        send_batch(&batch)?;
    }
    Ok(())
}

fn send_batch(inputs: &[INPUT]) -> Result<(), String> {
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(format!(
            "系统只接受了 {}/{} 个输入事件（目标程序可能以更高权限运行）",
            sent,
            inputs.len()
        ));
    }
    Ok(())
}

fn make_unicode_input(unit: u16, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn make_virtual_key(key: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(key),
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

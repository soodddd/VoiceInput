//! VoiceInput v2 — 剪贴板粘贴模块 (Windows)
//!
//! 实现流程：
//! 1. 保存当前剪贴板内容
//! 2. 将识别结果文本写入剪贴板
//! 3. 等待指定延迟
//! 4. 通过 SendInput 模拟 Ctrl+V 粘贴
//! 5. （可选）延迟 2 秒后恢复原剪贴板内容

#![cfg(windows)]

use std::thread;
use std::time::Duration;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    VK_CONTROL, VK_V,
};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};

/// 粘贴文本到当前焦点窗口。
///
/// # 参数
/// - `text`: 要粘贴的文本
/// - `delay_ms`: 写入剪贴板后、发送 Ctrl+V 前的等待时间（毫秒）
/// - `restore`: 是否在粘贴后恢复原剪贴板内容
pub fn paste_text(text: String, delay_ms: u64, restore: bool) -> Result<(), String> {
    if text.is_empty() {
        log::warn!("粘贴文本为空，跳过");
        return Ok(());
    }

    log::info!(
        "准备粘贴文本 ({} 字符), 延迟={}ms, 恢复剪贴板={}",
        text.chars().count(),
        delay_ms,
        restore
    );

    // 1. 保存原剪贴板内容
    let old_clipboard = get_clipboard_text().ok();

    // 2. 写入新文本到剪贴板
    set_clipboard_text(&text)?;

    // 3. 等待延迟
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }

    // 4. 模拟 Ctrl+V
    send_ctrl_v()?;

    // 5. 延迟恢复剪贴板
    if restore {
        if let Some(old_text) = old_clipboard {
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(2));
                log::info!("恢复原剪贴板内容");
                if let Err(e) = set_clipboard_text(&old_text) {
                    log::warn!("恢复剪贴板失败: {}", e);
                }
            });
        }
    }

    Ok(())
}

/// 获取当前剪贴板中的 Unicode 文本。
fn get_clipboard_text() -> Result<String, String> {
    unsafe {
        let _clip = ClipboardGuard::new()?;

        // GetClipboardData 返回 Result<HANDLE, Error>
        let handle = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
            Ok(h) if !h.is_invalid() => h,
            _ => return Ok(String::new()),
        };

        // GlobalLock 接收 HGLOBAL，返回 *mut c_void (windows 0.54 API)
        // GetClipboardData 返回 HANDLE，需转换为 HGLOBAL
        let ptr = GlobalLock(HGLOBAL(handle.0 as *mut std::ffi::c_void));
        if ptr.is_null() {
            return Err("GlobalLock 返回空指针".to_string());
        }
        let ptr_u16 = ptr as *const u16;

        // 计算 Unicode 字符串长度（null-terminated）
        let mut len = 0usize;
        while *ptr_u16.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(ptr_u16, len);
        let text = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(HGLOBAL(handle.0 as *mut std::ffi::c_void));
        Ok(text)
    }
}

/// 设置剪贴板文本（Unicode）。
fn set_clipboard_text(text: &str) -> Result<(), String> {
    unsafe {
        let _clip = ClipboardGuard::new()?;

        // 清空剪贴板
        let _ = EmptyClipboard();

        // 分配全局内存
        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0u16)).collect();
        let byte_size = utf16.len() * 2;

        // GlobalAlloc 返回 Result<HGLOBAL, Error>
        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_size)
            .map_err(|e| format!("GlobalAlloc 失败: {}", e))?;

        // GlobalLock 返回 *mut c_void (windows 0.54 API)
        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            return Err("GlobalLock 返回空指针".to_string());
        }
        let ptr_u16 = ptr as *mut u16;

        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr_u16, utf16.len());
        let _ = GlobalUnlock(hmem);

        // 设置剪贴板数据（转移内存所有权给系统）
        // SetClipboardData 接收 HANDLE，需将 HGLOBAL 转换为 HANDLE
        let result = SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(hmem.0 as isize));
        if result.is_err() {
            return Err("SetClipboardData 失败".to_string());
        }

        // 注意：SetClipboardData 成功后，系统接管内存所有权，不能 GlobalFree
        // windows 0.54 已移除 GlobalFree（已弃用）
        Ok(())
    }
}

/// 通过 SendInput 模拟 Ctrl+V 按键序列。
fn send_ctrl_v() -> Result<(), String> {
    unsafe {
        let inputs: [INPUT; 4] = [
            make_keyboard_input(VK_CONTROL.0, false),
            make_keyboard_input(VK_V.0, false),
            make_keyboard_input(VK_V.0, true),
            make_keyboard_input(VK_CONTROL.0, true),
        ];

        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent != 4 {
            return Err(format!("SendInput 只发送了 {} 个输入", sent));
        }
    }

    thread::sleep(Duration::from_millis(50));
    Ok(())
}

/// 构造一个键盘 INPUT 结构。
unsafe fn make_keyboard_input(vk: u16, key_up: bool) -> INPUT {
    let flags = if key_up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// RAII 守卫：确保 CloseClipboard 被调用。
struct ClipboardGuard;

impl ClipboardGuard {
    unsafe fn new() -> Result<Self, String> {
        let mut retries = 5;
        loop {
            if OpenClipboard(None).is_ok() {
                return Ok(ClipboardGuard);
            }
            retries -= 1;
            if retries == 0 {
                return Err("打开剪贴板失败 (重试 5 次)".to_string());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

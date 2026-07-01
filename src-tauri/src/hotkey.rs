//! VoiceInput v2 — 全局快捷键监听
//!
//! 使用 rdev 在独立线程监听全局键盘事件。
//! 支持的快捷键格式: "alt+v", "ctrl+shift+f" 等。
//! 按下 → emit "recording-start"，松开 → emit "recording-stop"。
//! 语言切换快捷键按下 → emit "language-cycle"。

use crate::config::AppConfig;
use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use tauri::{AppHandle, Emitter};

/// 当前活跃监听器的标志位存储。
///
/// 采用「代际失效」策略：调用 `start_hotkey_listener` 时，会先将旧标志位
/// 置为 false，使旧的 rdev 回调变为 no-op（rdev::listen 阻塞无法直接停止，
/// 因此只让它静默），然后安装新的标志位。
static ACTIVE_FLAG: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();

/// 获取全局活跃标志位存储（首次调用时初始化）。
fn active_flag_storage() -> &'static Mutex<Option<Arc<AtomicBool>>> {
    ACTIVE_FLAG.get_or_init(|| Mutex::new(None))
}

/// 修饰键组合
#[derive(Debug, Clone, PartialEq, Eq)]
struct HotkeyCombo {
    modifiers: Vec<Modifier>,
    key: Key,
}

/// 修饰键类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    Alt,
    Ctrl,
    Shift,
    Meta,
}

/// 跟踪当前按下的修饰键状态
#[derive(Debug, Default, Clone)]
struct ModifierState {
    alt: bool,
    ctrl: bool,
    shift: bool,
    meta: bool,
}

impl ModifierState {
    fn is_match(&self, target: &[Modifier]) -> bool {
        // 所有目标修饰键必须被按下
        for m in target {
            let pressed = match m {
                Modifier::Alt => self.alt,
                Modifier::Ctrl => self.ctrl,
                Modifier::Shift => self.shift,
                Modifier::Meta => self.meta,
            };
            if !pressed {
                return false;
            }
        }
        // 确保没有多余的修饰键（严格匹配）
        let extra_alt = self.alt && !target.contains(&Modifier::Alt);
        let extra_ctrl = self.ctrl && !target.contains(&Modifier::Ctrl);
        let extra_shift = self.shift && !target.contains(&Modifier::Shift);
        let extra_meta = self.meta && !target.contains(&Modifier::Meta);
        !(extra_alt || extra_ctrl || extra_shift || extra_meta)
    }

    fn update_on_press(&mut self, key: &Key) {
        match key {
            Key::Alt | Key::AltGr => self.alt = true,
            Key::ControlLeft | Key::ControlRight => self.ctrl = true,
            Key::ShiftLeft | Key::ShiftRight => self.shift = true,
            Key::MetaLeft | Key::MetaRight => self.meta = true,
            _ => {}
        }
    }

    fn update_on_release(&mut self, key: &Key) {
        match key {
            Key::Alt | Key::AltGr => self.alt = false,
            Key::ControlLeft | Key::ControlRight => self.ctrl = false,
            Key::ShiftLeft | Key::ShiftRight => self.shift = false,
            Key::MetaLeft | Key::MetaRight => self.meta = false,
            _ => {}
        }
    }
}

/// 解析快捷键字符串（如 "alt+v"）为 HotkeyCombo。
fn parse_hotkey(hotkey_str: &str) -> Result<HotkeyCombo, String> {
    let lower = hotkey_str.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(|s| s.trim()).collect();

    if parts.is_empty() {
        return Err(format!("快捷键格式无效: {}", hotkey_str));
    }

    let mut modifiers = Vec::new();
    for part in &parts[..parts.len() - 1] {
        let m = match *part {
            "alt" => Modifier::Alt,
            "ctrl" | "ctl" | "control" => Modifier::Ctrl,
            "shift" => Modifier::Shift,
            "meta" | "win" | "super" => Modifier::Meta,
            other => return Err(format!("未知修饰键: {}", other)),
        };
        modifiers.push(m);
    }

    let key_str = parts[parts.len() - 1];
    let key = parse_key(key_str)?;

    Ok(HotkeyCombo { modifiers, key })
}

/// 将字符串键名解析为 rdev::Key
fn parse_key(s: &str) -> Result<Key, String> {
    let key = match s {
        "a" => Key::KeyA,
        "b" => Key::KeyB,
        "c" => Key::KeyC,
        "d" => Key::KeyD,
        "e" => Key::KeyE,
        "f" => Key::KeyF,
        "g" => Key::KeyG,
        "h" => Key::KeyH,
        "i" => Key::KeyI,
        "j" => Key::KeyJ,
        "k" => Key::KeyK,
        "l" => Key::KeyL,
        "m" => Key::KeyM,
        "n" => Key::KeyN,
        "o" => Key::KeyO,
        "p" => Key::KeyP,
        "q" => Key::KeyQ,
        "r" => Key::KeyR,
        "s" => Key::KeyS,
        "t" => Key::KeyT,
        "u" => Key::KeyU,
        "v" => Key::KeyV,
        "w" => Key::KeyW,
        "x" => Key::KeyX,
        "y" => Key::KeyY,
        "z" => Key::KeyZ,
        "0" => Key::Num0,
        "1" => Key::Num1,
        "2" => Key::Num2,
        "3" => Key::Num3,
        "4" => Key::Num4,
        "5" => Key::Num5,
        "6" => Key::Num6,
        "7" => Key::Num7,
        "8" => Key::Num8,
        "9" => Key::Num9,
        "space" => Key::Space,
        "enter" | "return" => Key::Return,
        "esc" | "escape" => Key::Escape,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        other => return Err(format!("未知按键: {}", other)),
    };
    Ok(key)
}

/// 启动全局快捷键监听线程。
///
/// 采用「代际失效」策略：先将旧的活跃标志位置为 false，使旧 rdev 回调
/// 变为 no-op（rdev::listen 阻塞无法直接停止），然后安装新的标志位并
/// 启动新线程。因此重复调用此函数即可实现热重载。
pub fn start_hotkey_listener(app: AppHandle, config: AppConfig) {
    // 失效旧的监听器
    {
        let storage = active_flag_storage();
        if let Ok(mut guard) = storage.lock() {
            if let Some(old_flag) = guard.take() {
                old_flag.store(false, Ordering::SeqCst);
                log::info!("已停用旧的快捷键监听器");
            }
        }
    }

    // 创建新的活跃标志位并存储
    let active = Arc::new(AtomicBool::new(true));
    {
        let storage = active_flag_storage();
        if let Ok(mut guard) = storage.lock() {
            *guard = Some(active.clone());
        }
    }

    let main_hotkey = match parse_hotkey(&config.hotkey) {
        Ok(k) => k,
        Err(e) => {
            log::error!("解析主快捷键失败: {} (配置值: {})", e, config.hotkey);
            return;
        }
    };

    let lang_hotkey = match parse_hotkey(&config.language_hotkey) {
        Ok(k) => k,
        Err(e) => {
            log::error!(
                "解析语言快捷键失败: {} (配置值: {})",
                e,
                config.language_hotkey
            );
            return;
        }
    };

    log::info!(
        "启动快捷键监听: 主键={:?}+{:?}, 语言键={:?}+{:?}",
        main_hotkey.modifiers,
        main_hotkey.key,
        lang_hotkey.modifiers,
        lang_hotkey.key
    );

    // 防抖标志：防止 KeyPress 重复触发
    let main_active = Arc::new(AtomicBool::new(false));
    let lang_active = Arc::new(AtomicBool::new(false));

    // 修饰键状态跟踪
    let mod_state = Arc::new(Mutex::new(ModifierState::default()));

    let app_clone = app.clone();
    let main_active_clone = main_active.clone();
    let lang_active_clone = lang_active.clone();
    let mod_state_clone = mod_state.clone();
    let active_clone = active.clone();

    thread::spawn(move || {
        let callback = move |event: Event| {
            // 若本代监听器已被停用，直接忽略所有事件
            if !active_clone.load(Ordering::SeqCst) {
                return;
            }

            // 更新修饰键状态
            match event.event_type {
                EventType::KeyPress(ref key) => {
                    if let Ok(mut state) = mod_state_clone.lock() {
                        state.update_on_press(key);
                    }
                }
                EventType::KeyRelease(ref key) => {
                    if let Ok(mut state) = mod_state_clone.lock() {
                        state.update_on_release(key);
                    }
                }
                _ => {}
            }

            match event.event_type {
                EventType::KeyPress(key) => {
                    // 获取当前修饰键状态
                    let state_match = {
                        let state = mod_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                        state.clone()
                    };

                    // 主快捷键按下
                    if key == main_hotkey.key
                        && state_match.is_match(&main_hotkey.modifiers)
                        && !main_active_clone.load(Ordering::SeqCst)
                    {
                        main_active_clone.store(true, Ordering::SeqCst);
                        log::info!("主快捷键按下");
                        let _ = app_clone.emit("recording-start", ());
                    }

                    // 语言快捷键按下
                    if key == lang_hotkey.key
                        && state_match.is_match(&lang_hotkey.modifiers)
                        && !lang_active_clone.load(Ordering::SeqCst)
                    {
                        lang_active_clone.store(true, Ordering::SeqCst);
                        log::info!("语言快捷键按下");
                        let _ = app_clone.emit("language-cycle", ());
                    }
                }
                EventType::KeyRelease(key) => {
                    // 主快捷键松开
                    if key == main_hotkey.key && main_active_clone.load(Ordering::SeqCst) {
                        main_active_clone.store(false, Ordering::SeqCst);
                        log::info!("主快捷键松开");
                        let _ = app_clone.emit("recording-stop", ());
                    }

                    // 语言快捷键松开（重置防抖）
                    if key == lang_hotkey.key {
                        lang_active_clone.store(false, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        };

        if let Err(e) = listen(callback) {
            log::error!("rdev 监听失败: {:?}", e);
        }
    });
}

/// 停止当前活跃的快捷键监听器。
///
/// 将当前活跃标志位置为 false 使旧 rdev 回调变为 no-op，
/// 并清空存储，表示没有活跃监听器。
pub fn stop_hotkey_listener() {
    let storage = active_flag_storage();
    if let Ok(mut guard) = storage.lock() {
        if let Some(old_flag) = guard.take() {
            old_flag.store(false, Ordering::SeqCst);
            log::info!("已停用快捷键监听器");
        }
    }
}

/// 重启快捷键监听（热重载配置）。
///
/// 先停用旧回调，再使用最新配置启动新回调。
pub fn restart_hotkey_listener(app: AppHandle, config: AppConfig) {
    log::info!("重启快捷键监听以应用新配置...");
    stop_hotkey_listener();
    start_hotkey_listener(app, config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hotkey_alt_v() {
        let combo = parse_hotkey("alt+v").unwrap();
        assert_eq!(combo.modifiers, vec![Modifier::Alt]);
        assert_eq!(combo.key, Key::KeyV);
    }

    #[test]
    fn test_parse_hotkey_ctrl_shift_f() {
        let combo = parse_hotkey("ctrl+shift+f").unwrap();
        assert_eq!(combo.modifiers, vec![Modifier::Ctrl, Modifier::Shift]);
        assert_eq!(combo.key, Key::KeyF);
    }

    #[test]
    fn test_parse_hotkey_invalid() {
        assert!(parse_hotkey("xyz+a").is_err());
    }

    #[test]
    fn test_modifier_state_match() {
        let mut state = ModifierState::default();
        state.alt = true;

        let target = vec![Modifier::Alt];
        assert!(state.is_match(&target));

        state.ctrl = true;
        assert!(!state.is_match(&target)); // extra ctrl
    }

    #[test]
    fn test_modifier_state_update() {
        let mut state = ModifierState::default();
        state.update_on_press(&Key::Alt);
        assert!(state.alt);
        state.update_on_release(&Key::Alt);
        assert!(!state.alt);
    }
}

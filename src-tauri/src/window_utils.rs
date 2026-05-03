//! Optional Windows click-through (extended window styles).
//! Default: no behavior change; use command `set_click_through` to toggle.

#[cfg(windows)]
use tauri::WebviewWindow;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT,
};

#[cfg(windows)]
fn hwnd_from_window(window: &WebviewWindow) -> Result<HWND, String> {
    let raw = window.hwnd().map_err(|e| e.to_string())?;
    Ok(HWND(raw.0))
}

#[cfg(windows)]
pub fn set_click_through(window: &WebviewWindow, enabled: bool) -> Result<(), String> {
    let hwnd = hwnd_from_window(window)?;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if style == 0 {
            return Err("GetWindowLongPtrW returned 0".into());
        }
        let mut new_style = style as u32;
        if enabled {
            new_style |= WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0;
        } else {
            new_style &= !(WS_EX_TRANSPARENT.0);
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style as isize);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_click_through(_window: &tauri::WebviewWindow, _enabled: bool) -> Result<(), String> {
    Err("click-through is only supported on Windows".into())
}

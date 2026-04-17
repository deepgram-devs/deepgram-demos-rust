use std::ptr::copy_nonoverlapping;

use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

const CF_UNICODETEXT: u32 = 13;

pub fn copy_text(text: &str) -> Result<(), String> {
    let utf16 = text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
    let bytes = utf16.len() * std::mem::size_of::<u16>();

    unsafe {
        OpenClipboard(Some(HWND(std::ptr::null_mut())))
            .map_err(|e| format!("Failed to open clipboard: {e}"))?;
        EmptyClipboard().map_err(|e| format!("Failed to empty clipboard: {e}"))?;

        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes)
            .map_err(|e| format!("Failed to allocate clipboard memory: {e}"))?;
        if handle.is_invalid() {
            let _ = CloseClipboard();
            return Err("Failed to allocate clipboard memory".to_string());
        }

        let ptr = GlobalLock(handle) as *mut u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err("Failed to lock clipboard memory".to_string());
        }

        copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
        let _ = GlobalUnlock(handle);

        SetClipboardData(CF_UNICODETEXT, Some(HANDLE(handle.0)))
            .map_err(|e| format!("Failed to set clipboard data: {e}"))?;
        CloseClipboard().map_err(|e| format!("Failed to close clipboard: {e}"))?;
    }

    Ok(())
}

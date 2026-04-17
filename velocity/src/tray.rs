use png::{ColorType, Decoder};
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Shell::*,
    Win32::UI::WindowsAndMessaging::*,
};

static ICON_PNG: &[u8] = include_bytes!("../assets/deepgram-icon.png");

pub const WM_TRAY: u32 = WM_APP + 1;
pub const WM_APP_REFRESH_TRAY: u32 = WM_APP + 2;
pub const WM_APP_RELOAD_CONFIG: u32 = WM_APP + 3;
const IDM_SETTINGS: usize = 1001;
const IDM_KEEP_TALKING: usize = 1002;
const IDM_STREAMING: usize = 1003;
const IDM_QUIT: usize = 1004;
const IDM_RECENT_BASE: usize = 2000;
const TRAY_ID: u32 = 1;

/// Creates a message-only window and registers the system tray icon.
pub fn create_tray_window() -> HWND {
    unsafe {
        let hmodule = GetModuleHandleW(None).unwrap();
        let hinstance = HINSTANCE(hmodule.0);
        let class_name = w!("VelocityTray");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE,
            class_name,
            w!("Velocity"),
            WS_OVERLAPPED,
            0, 0, 0, 0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance),
            None,
        )
        .expect("Failed to create tray window");

        add_tray_icon(hwnd);
        hwnd
    }
}

unsafe fn load_icon() -> HICON {
    // Decode the embedded PNG into BGRA pixels.
    let decoder = Decoder::new(std::io::Cursor::new(ICON_PNG));
    let mut reader = match decoder.read_info() {
        Ok(r) => r,
        Err(_) => return HICON::default(),
    };
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = match reader.next_frame(&mut buf) {
        Ok(i) => i,
        Err(_) => return HICON::default(),
    };

    let w = info.width as i32;
    let h = info.height as i32;
    let src = &buf[..info.buffer_size()];

    // Windows 32bpp icons use BGRA byte order.
    let mut bgra: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
    match info.color_type {
        ColorType::Rgba => {
            for p in src.chunks_exact(4) {
                bgra.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
        ColorType::Rgb => {
            for p in src.chunks_exact(3) {
                bgra.extend_from_slice(&[p[2], p[1], p[0], 0xFF]);
            }
        }
        _ => return HICON::default(),
    }

    // Create a 32bpp top-down DIB section and copy pixel data into it.
    let bmi_header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h, // negative = top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let bmi = BITMAPINFO {
        bmiHeader: bmi_header,
        bmiColors: [RGBQUAD::default()],
    };

    let hdc = CreateCompatibleDC(None);
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbm_color = CreateDIBSection(Some(hdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
        .unwrap_or_default();
    if !hbm_color.is_invalid() && !bits.is_null() {
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
    }
    let _ = DeleteDC(hdc);

    // All-zeros monochrome mask means fully opaque (alpha comes from the colour bitmap).
    let mask_bytes = vec![0u8; ((w * h + 7) / 8) as usize];
    let hbm_mask = CreateBitmap(w, h, 1, 1, Some(mask_bytes.as_ptr() as _));

    let icon_info = ICONINFO {
        fIcon: BOOL(1),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };
    let hicon = CreateIconIndirect(&icon_info).unwrap_or_default();
    let _ = DeleteObject(hbm_color.into());
    let _ = DeleteObject(hbm_mask.into());
    hicon
}

unsafe fn add_tray_icon(hwnd: HWND) {
    let hicon = load_icon();

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: hicon,
        ..Default::default()
    };

    let tip = tray_tooltip();
    let tip_utf16: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
    let len = tip_utf16.len().min(nid.szTip.len());
    nid.szTip[..len].copy_from_slice(&tip_utf16[..len]);

    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
}

pub fn refresh_tray(hwnd: HWND) {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_TIP,
            ..Default::default()
        };

        let tip = tray_tooltip();
        let tip_utf16: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
        let len = tip_utf16.len().min(nid.szTip.len());
        nid.szTip[..len].copy_from_slice(&tip_utf16[..len]);
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

pub fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                show_context_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                crate::settings::show_settings_window();
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            crate::state::global().with_hotkeys(|hotkeys| hotkeys.handle(wparam.0 as i32));
            LRESULT(0)
        }
        WM_APP_REFRESH_TRAY => {
            refresh_tray(hwnd);
            LRESULT(0)
        }
        WM_APP_RELOAD_CONFIG => {
            apply_reloaded_config();
            refresh_tray(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 {
                x if x == IDM_SETTINGS => crate::settings::show_settings_window(),
                x if x == IDM_KEEP_TALKING => {
                    let _ = crate::state::global()
                        .with_hotkeys(|hotkeys| hotkeys.handle(crate::hotkey::HOTKEY_KT));
                }
                x if x == IDM_STREAMING => {
                    let _ = crate::state::global()
                        .with_hotkeys(|hotkeys| hotkeys.handle(crate::hotkey::HOTKEY_STREAM));
                }
                x if x == IDM_QUIT => {
                    remove_tray_icon(hwnd);
                    PostQuitMessage(0);
                }
                x if x >= IDM_RECENT_BASE => {
                    let index = x - IDM_RECENT_BASE;
                    let app = crate::state::global();
                    if app.select_history(index).is_some() {
                        app.resend_selected();
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_context_menu(hwnd: HWND) {
    let hmenu = CreatePopupMenu().unwrap();
    let app = crate::state::global();
    let status_line = if let Some(error) = app.last_error() {
        format!("Velocity - {}", truncate_menu_text(&error, 40))
    } else if app.is_streaming() {
        "Velocity - Streaming active".to_string()
    } else if app.is_keep_talking() || app.is_recording() {
        "Velocity - Recording active".to_string()
    } else {
        "Velocity - Idle".to_string()
    };
    let status_wide = to_wide(&status_line);
    let _ = AppendMenuW(hmenu, MF_STRING | MF_DISABLED, 0, PCWSTR(status_wide.as_ptr()));
    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());

    let keep_label = if app.is_keep_talking() {
        "Stop keep talking"
    } else {
        "Start keep talking"
    };
    let stream_label = if app.is_streaming() {
        "Stop streaming"
    } else {
        "Start streaming"
    };

    let _ = AppendMenuW(hmenu, MF_STRING, IDM_KEEP_TALKING, PCWSTR(to_wide(keep_label).as_ptr()));
    let _ = AppendMenuW(hmenu, MF_STRING, IDM_STREAMING, PCWSTR(to_wide(stream_label).as_ptr()));
    let _ = AppendMenuW(hmenu, MF_STRING, IDM_SETTINGS, w!("Settings"));

    let recent_menu = CreatePopupMenu().unwrap();
    let selected_recent = app.selected_history_index();
    let recent_entries = app.recent_entries();
    for (index, entry) in recent_entries.iter().enumerate() {
        let label = format_recent_label(index, &entry.text);
        let flags = if Some(index) == selected_recent {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let _ = AppendMenuW(
            recent_menu,
            flags,
            IDM_RECENT_BASE + index,
            PCWSTR(to_wide(&label).as_ptr()),
        );
    }
    if recent_entries.is_empty() {
        let _ = AppendMenuW(recent_menu, MF_STRING | MF_DISABLED, 0, w!("No recent transcripts"));
    }
    let _ = AppendMenuW(hmenu, MF_POPUP, recent_menu.0 as usize, w!("Recent transcripts"));

    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(hmenu, MF_STRING, IDM_QUIT, w!("Quit Velocity"));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);

    SetForegroundWindow(hwnd);
    TrackPopupMenu(
        hmenu,
        TPM_BOTTOMALIGN | TPM_LEFTALIGN,
        pt.x,
        pt.y,
        None,
        hwnd,
        None,
    );
    let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(hmenu);
}

fn tray_tooltip() -> String {
    let app = crate::state::global();
    if let Some(error) = app.last_error() {
        format!("Velocity - Error: {}", truncate_menu_text(&error, 40))
    } else if app.is_streaming() {
        "Velocity - Streaming active".to_string()
    } else if app.is_keep_talking() || app.is_recording() {
        "Velocity - Recording active".to_string()
    } else {
        "Velocity - Idle".to_string()
    }
}

fn format_recent_label(index: usize, text: &str) -> String {
    format!("{}: {}", index + 1, truncate_menu_text(text, 48))
}

fn truncate_menu_text(text: &str, limit: usize) -> String {
    let truncated = text.trim().replace('\n', " ");
    if truncated.chars().count() <= limit {
        truncated
    } else {
        format!("{}...", truncated.chars().take(limit).collect::<String>())
    }
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn apply_reloaded_config() {
    let app = crate::state::global();
    let path = crate::config::config_path();
    match crate::config::load_from_path(&path) {
        Ok(loaded) => {
            if loaded.modified_at == app.config_modified_at() {
                return;
            }
            let previous = app.config();
            if let Some(result) = app.with_hotkeys(|hotkeys| hotkeys.apply_config(loaded.config.hotkeys.clone())) {
                if let Err(error) = result {
                    let _ = app.with_hotkeys(|hotkeys| hotkeys.apply_config(previous.hotkeys.clone()));
                    app.set_error(format!("Config reload rejected: {error}"));
                    return;
                }
            }
            app.apply_config(loaded.config, loaded.modified_at);
        }
        Err(error) => app.set_error(format!("Config reload rejected: {error}")),
    }
}

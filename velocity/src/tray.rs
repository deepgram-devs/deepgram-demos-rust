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
const IDM_QUIT: usize = 1001;
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

    let tip = "Velocity";
    let tip_utf16: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
    let len = tip_utf16.len().min(nid.szTip.len());
    nid.szTip[..len].copy_from_slice(&tip_utf16[..len]);

    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
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
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            if wparam.0 == IDM_QUIT {
                remove_tray_icon(hwnd);
                PostQuitMessage(0);
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

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::{GetStockObject, HBRUSH, WHITE_BRUSH},
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
};

/// Shows a simple dialog prompting the user to enter their Deepgram API key.
/// Returns Some(key) on OK, or None if the user cancelled.
pub fn prompt_for_api_key() -> Option<String> {
    unsafe { show_dialog() }
}

std::thread_local! {
    static DIALOG_RESULT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

const IDC_EDIT: i32 = 100;
const IDC_OK: i32 = 101;
const IDC_CANCEL: i32 = 102;

// SS_LEFT = 0, not exported by the windows crate in all feature sets
const SS_LEFT: u32 = 0;

unsafe fn show_dialog() -> Option<String> {
    let hmodule = GetModuleHandleW(None).unwrap();
    let hinstance = HINSTANCE(hmodule.0);

    let class_name = w!("VelocityApiKeyDlg");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(dialog_proc),
        hInstance: hinstance,
        lpszClassName: class_name,
        hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
        ..Default::default()
    };
    RegisterClassW(&wc);

    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class_name,
        w!("Velocity \u{2014} Enter Deepgram API Key"),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
        CW_USEDEFAULT, CW_USEDEFAULT, 440, 160,
        None, None, Some(hinstance), None,
    )
    .unwrap();

    // Label
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        w!("Enter your Deepgram API key:"),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SS_LEFT),
        10, 10, 400, 20,
        Some(hwnd), None, Some(hinstance), None,
    )
    .unwrap();

    // Edit control
    CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
        10, 36, 400, 24,
        Some(hwnd), Some(HMENU(IDC_EDIT as isize as *mut _)), Some(hinstance), None,
    )
    .unwrap();

    // OK button
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("OK"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
        240, 78, 80, 28,
        Some(hwnd), Some(HMENU(IDC_OK as isize as *mut _)), Some(hinstance), None,
    )
    .unwrap();

    // Cancel button
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Cancel"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        330, 78, 80, 28,
        Some(hwnd), Some(HMENU(IDC_CANCEL as isize as *mut _)), Some(hinstance), None,
    )
    .unwrap();

    ShowWindow(hwnd, SW_SHOW);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        if !IsDialogMessageW(hwnd, &msg).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    DIALOG_RESULT.with(|r| r.borrow_mut().take())
}

unsafe extern "system" fn dialog_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            match id {
                IDC_OK => {
                    if let Ok(edit) = GetDlgItem(Some(hwnd), IDC_EDIT) {
                        let mut buf = [0u16; 512];
                        let len = GetWindowTextW(edit, &mut buf);
                        if len > 0 {
                            let key = String::from_utf16_lossy(&buf[..len as usize]);
                            let key = key.trim().to_string();
                            if !key.is_empty() {
                                DIALOG_RESULT.with(|r| *r.borrow_mut() = Some(key));
                            }
                        }
                    }
                    let _ = DestroyWindow(hwnd);
                    PostQuitMessage(0);
                }
                IDC_CANCEL => {
                    let _ = DestroyWindow(hwnd);
                    PostQuitMessage(0);
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

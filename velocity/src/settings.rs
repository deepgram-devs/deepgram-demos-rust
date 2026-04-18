use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::{GetStockObject, HBRUSH, WHITE_BRUSH},
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
};

use crate::audio::{self, AudioMeter};
use crate::config::{self, Config, OutputMode};
use crate::deepgram;
use crate::hotkey;
use crate::state;

pub const WM_APP_SETTINGS_REFRESH: u32 = WM_APP + 30;
const BST_CHECKED: usize = 1;
const BST_UNCHECKED: usize = 0;

const IDC_API_KEY: i32 = 100;
const IDC_MODEL: i32 = 101;
const IDC_LANGUAGE: i32 = 102;
const IDC_SMART_FORMAT: i32 = 103;
const IDC_KEY_TERMS: i32 = 104;
const IDC_PUSH_TO_TALK: i32 = 105;
const IDC_KEEP_TALKING: i32 = 106;
const IDC_STREAMING: i32 = 107;
const IDC_RESEND: i32 = 108;
const IDC_AUDIO_INPUT: i32 = 109;
const IDC_MIC_ACTIVITY: i32 = 110;
const IDC_HISTORY_LIMIT: i32 = 111;
const IDC_OUTPUT_MODE: i32 = 112;
const IDC_APPEND_NEWLINE: i32 = 113;
const IDC_STATUS: i32 = 114;
const IDC_SAVE: i32 = 115;
const IDC_CANCEL: i32 = 116;

const SETTINGS_TIMER_ID: usize = 1;

struct SettingsWindowState {
    meter: Option<AudioMeter>,
    meter_device: Option<String>,
}

pub fn show_settings_window() {
    if crate::sidecar_ui::launch_settings().unwrap_or(false) {
        return;
    }

    unsafe {
        let app = state::global();
        if let Some(hwnd) = app.settings_hwnd() {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            return;
        }

        let hmodule = GetModuleHandleW(None).unwrap();
        let hinstance = HINSTANCE(hmodule.0);
        let class_name = w!("VelocitySettings");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(settings_wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name,
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            class_name,
            w!("Velocity Settings"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            620,
            760,
            None,
            None,
            Some(hinstance),
            None,
        )
        .expect("Failed to create settings window");

        app.set_settings_hwnd(Some(hwnd));
        ShowWindow(hwnd, SW_SHOW);
    }
}

unsafe extern "system" fn settings_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            initialize_window(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            match id {
                IDC_SAVE => {
                    save_settings(hwnd);
                    LRESULT(0)
                }
                IDC_CANCEL => {
                    DestroyWindow(hwnd).ok();
                    LRESULT(0)
                }
                IDC_AUDIO_INPUT => {
                    if ((wparam.0 >> 16) & 0xFFFF) as u16 == CBN_SELCHANGE as u16 {
                        reset_meter(hwnd);
                    }
                    LRESULT(0)
                }
                IDC_MODEL => {
                    if ((wparam.0 >> 16) & 0xFFFF) as u16 == CBN_SELCHANGE as u16 {
                        update_language_options_for_selected_model(hwnd);
                    }
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
        WM_TIMER => {
            if wparam.0 == SETTINGS_TIMER_ID {
                update_meter(hwnd);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_APP_SETTINGS_REFRESH => {
            populate_controls(hwnd, &state::global().config());
            update_status_text(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let app = state::global();
            app.set_settings_hwnd(None);
            let _ = KillTimer(Some(hwnd), SETTINGS_TIMER_ID);
            drop_window_state(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn initialize_window(hwnd: HWND) {
    let state = Box::new(SettingsWindowState {
        meter: None,
        meter_device: None,
    });
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

    let hmodule = GetModuleHandleW(None).unwrap();
    let hinstance = HINSTANCE(hmodule.0);

    let mut y = 12;
    create_label(hwnd, hinstance, "Deepgram API key", 12, y, 150, 18);
    create_edit(hwnd, hinstance, IDC_API_KEY, 170, y - 2, 420, 22, false);
    y += 30;

    create_label(hwnd, hinstance, "Model", 12, y, 150, 18);
    create_combo_box(hwnd, hinstance, IDC_MODEL, 170, y - 4, 180, 120);
    create_checkbox(hwnd, hinstance, IDC_SMART_FORMAT, "Enable smart formatting", 370, y - 2, 220, 22);
    y += 30;

    create_label(hwnd, hinstance, "Language", 12, y, 150, 18);
    create_combo_box(hwnd, hinstance, IDC_LANGUAGE, 170, y - 4, 420, 320);
    y += 30;

    create_label(hwnd, hinstance, "Key terms (comma-separated)", 12, y, 150, 18);
    create_edit(hwnd, hinstance, IDC_KEY_TERMS, 170, y - 2, 420, 22, false);
    y += 30;

    create_label(hwnd, hinstance, "Push to talk hotkey", 12, y, 150, 18);
    create_edit(hwnd, hinstance, IDC_PUSH_TO_TALK, 170, y - 2, 180, 22, false);
    create_label(hwnd, hinstance, "Keep talking hotkey", 360, y, 140, 18);
    create_edit(hwnd, hinstance, IDC_KEEP_TALKING, 500, y - 2, 90, 22, false);
    y += 30;

    create_label(hwnd, hinstance, "Streaming hotkey", 12, y, 150, 18);
    create_edit(hwnd, hinstance, IDC_STREAMING, 170, y - 2, 180, 22, false);
    create_label(hwnd, hinstance, "Resend recent hotkey", 360, y, 140, 18);
    create_edit(hwnd, hinstance, IDC_RESEND, 500, y - 2, 90, 22, false);
    y += 30;

    create_label(hwnd, hinstance, "Audio input device", 12, y, 150, 18);
    create_combo_box(hwnd, hinstance, IDC_AUDIO_INPUT, 170, y - 4, 300, 320);
    y += 30;

    create_label(hwnd, hinstance, "Mic activity", 12, y, 150, 18);
    create_label_with_id(hwnd, hinstance, IDC_MIC_ACTIVITY, "", 170, y, 250, 18).unwrap();
    y += 30;

    create_label(hwnd, hinstance, "Recent history limit", 12, y, 150, 18);
    create_edit(hwnd, hinstance, IDC_HISTORY_LIMIT, 170, y - 2, 90, 22, false);
    create_label(hwnd, hinstance, "Output mode", 300, y, 90, 18);
    create_combo_box(hwnd, hinstance, IDC_OUTPUT_MODE, 390, y - 4, 200, 120);
    y += 30;

    create_checkbox(hwnd, hinstance, IDC_APPEND_NEWLINE, "Append newline after transcript", 170, y - 2, 260, 22);
    y += 36;

    create_label_with_id(hwnd, hinstance, IDC_STATUS, "", 12, y, 578, 40).unwrap();
    y += 50;

    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Save"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
        420,
        y,
        80,
        28,
        Some(hwnd),
        Some(HMENU(IDC_SAVE as isize as *mut _)),
        Some(hinstance),
        None,
    )
    .unwrap();

    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Cancel"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        510,
        y,
        80,
        28,
        Some(hwnd),
        Some(HMENU(IDC_CANCEL as isize as *mut _)),
        Some(hinstance),
        None,
    )
    .unwrap();

    populate_model_options(hwnd);
    populate_output_modes(hwnd);
    populate_audio_devices(hwnd);
    populate_controls(hwnd, &state::global().config());
    SetTimer(Some(hwnd), SETTINGS_TIMER_ID, 150, None);
}

unsafe fn populate_model_options(hwnd: HWND) {
    let combo = GetDlgItem(Some(hwnd), IDC_MODEL).unwrap();
    send_message(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for model in deepgram::supported_models() {
        let wide = to_wide(model);
        send_message(combo, CB_ADDSTRING, WPARAM(0), LPARAM(wide.as_ptr() as isize));
    }
}

unsafe fn populate_output_modes(hwnd: HWND) {
    let combo = GetDlgItem(Some(hwnd), IDC_OUTPUT_MODE).unwrap();
    send_message(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for mode in OutputMode::all() {
        let wide = to_wide(mode.as_label());
        send_message(combo, CB_ADDSTRING, WPARAM(0), LPARAM(wide.as_ptr() as isize));
    }
}

unsafe fn populate_audio_devices(hwnd: HWND) {
    let combo = GetDlgItem(Some(hwnd), IDC_AUDIO_INPUT).unwrap();
    send_message(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for device in audio::list_input_devices() {
        let wide = to_wide(&device.name);
        send_message(combo, CB_ADDSTRING, WPARAM(0), LPARAM(wide.as_ptr() as isize));
    }
}

unsafe fn populate_controls(hwnd: HWND, config: &Config) {
    set_control_text(hwnd, IDC_API_KEY, config.api_key.as_deref().unwrap_or(""));
    select_combo_value(hwnd, IDC_MODEL, &config.model);
    populate_language_options(hwnd, &config.model, config.language.as_deref());
    set_control_text(hwnd, IDC_KEY_TERMS, &format_key_terms_display(&config.key_terms));
    set_control_text(hwnd, IDC_PUSH_TO_TALK, &config.hotkeys.push_to_talk);
    set_control_text(hwnd, IDC_KEEP_TALKING, &config.hotkeys.keep_talking);
    set_control_text(hwnd, IDC_STREAMING, &config.hotkeys.streaming);
    set_control_text(hwnd, IDC_RESEND, &config.hotkeys.resend_selected);
    set_control_text(hwnd, IDC_HISTORY_LIMIT, &config.history_limit.to_string());

    let smart = GetDlgItem(Some(hwnd), IDC_SMART_FORMAT).unwrap();
    send_message(
        smart,
        BM_SETCHECK,
        WPARAM(if config.smart_format { BST_CHECKED as usize } else { BST_UNCHECKED as usize }),
        LPARAM(0),
    );

    let append = GetDlgItem(Some(hwnd), IDC_APPEND_NEWLINE).unwrap();
    send_message(
        append,
        BM_SETCHECK,
        WPARAM(if config.append_newline { BST_CHECKED as usize } else { BST_UNCHECKED as usize }),
        LPARAM(0),
    );

    select_combo_value(hwnd, IDC_AUDIO_INPUT, config.audio_input.as_deref().unwrap_or("Default system input"));
    select_combo_value(hwnd, IDC_OUTPUT_MODE, config.output_mode.as_label());
    update_status_text(hwnd);
}

unsafe fn populate_language_options(hwnd: HWND, model: &str, selected_language: Option<&str>) {
    let combo = GetDlgItem(Some(hwnd), IDC_LANGUAGE).unwrap();
    send_message(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));

    let default_label = to_wide(deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL);
    send_message(combo, CB_ADDSTRING, WPARAM(0), LPARAM(default_label.as_ptr() as isize));

    for option in deepgram::languages_for_model(model) {
        let display = deepgram::language_display(option);
        let wide = to_wide(&display);
        send_message(combo, CB_ADDSTRING, WPARAM(0), LPARAM(wide.as_ptr() as isize));
    }

    if let Some(language) = selected_language.and_then(|value| deepgram::normalize_language(model, Some(value)).ok().flatten()) {
        for option in deepgram::languages_for_model(model) {
            if option.code.eq_ignore_ascii_case(&language) {
                let display = deepgram::language_display(option);
                select_combo_value(hwnd, IDC_LANGUAGE, &display);
                return;
            }
        }
    }

    select_combo_value(hwnd, IDC_LANGUAGE, deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL);
}

unsafe fn update_language_options_for_selected_model(hwnd: HWND) {
    let model = get_selected_combo_text(hwnd, IDC_MODEL);
    let current_language = get_selected_combo_text(hwnd, IDC_LANGUAGE);
    let selected_language = deepgram::language_code_from_display(&model, &current_language);
    populate_language_options(hwnd, &model, selected_language.as_deref());
}

unsafe fn update_status_text(hwnd: HWND) {
    let app = state::global();
    let status = if let Some(error) = app.last_error() {
        error
    } else {
        let active = app.config();
        format!(
            "Current mode: recording={} keep-talking={} streaming={} selected mic={}",
            app.is_recording(),
            app.is_keep_talking(),
            app.is_streaming(),
            active.audio_input.as_deref().unwrap_or("Default system input")
        )
    };
    set_control_text(hwnd, IDC_STATUS, &status);
}

unsafe fn save_settings(hwnd: HWND) {
    let app = state::global();
    let previous = app.config();
    let mut updated = previous.clone();

    let api_key = get_control_text(hwnd, IDC_API_KEY);
    updated.api_key = (!api_key.trim().is_empty()).then(|| api_key.trim().to_string());
    updated.model = get_selected_combo_text(hwnd, IDC_MODEL);
    updated.language = deepgram::language_code_from_display(
        &updated.model,
        &get_selected_combo_text(hwnd, IDC_LANGUAGE),
    );
    updated.smart_format = is_checked(hwnd, IDC_SMART_FORMAT);
    updated.key_terms = parse_key_terms_text(&get_control_text(hwnd, IDC_KEY_TERMS));
    updated.hotkeys.push_to_talk = get_control_text(hwnd, IDC_PUSH_TO_TALK);
    updated.hotkeys.keep_talking = get_control_text(hwnd, IDC_KEEP_TALKING);
    updated.hotkeys.streaming = get_control_text(hwnd, IDC_STREAMING);
    updated.hotkeys.resend_selected = get_control_text(hwnd, IDC_RESEND);
    updated.audio_input = Some(get_selected_combo_text(hwnd, IDC_AUDIO_INPUT))
        .filter(|value| value != "Default system input");
    updated.history_limit = get_control_text(hwnd, IDC_HISTORY_LIMIT)
        .trim()
        .parse::<usize>()
        .unwrap_or(0);
    updated.output_mode = match get_selected_combo_text(hwnd, IDC_OUTPUT_MODE).as_str() {
        "Copy to clipboard" => OutputMode::Clipboard,
        "Paste clipboard" => OutputMode::Paste,
        _ => OutputMode::DirectInput,
    };
    updated.append_newline = is_checked(hwnd, IDC_APPEND_NEWLINE);

    if let Err(error) = updated.normalize() {
        set_control_text(hwnd, IDC_STATUS, &error);
        return;
    }
    if let Err(error) = validate_hotkeys(&updated) {
        set_control_text(hwnd, IDC_STATUS, &error);
        return;
    }

    let hotkey_result = app.with_hotkeys(|hotkeys| hotkeys.apply_config(updated.hotkeys.clone()));
    if let Some(Err(error)) = hotkey_result {
        let _ = app.with_hotkeys(|hotkeys| hotkeys.apply_config(previous.hotkeys.clone()));
        set_control_text(hwnd, IDC_STATUS, &error);
        return;
    }

    if let Err(error) = config::save(&updated) {
        let _ = app.with_hotkeys(|hotkeys| hotkeys.apply_config(previous.hotkeys.clone()));
        set_control_text(hwnd, IDC_STATUS, &error);
        return;
    }

    let modified_at = std::fs::metadata(config::config_path())
        .ok()
        .and_then(|meta| meta.modified().ok());
    app.apply_config(updated, modified_at);
    set_control_text(hwnd, IDC_STATUS, "Settings saved");
    reset_meter(hwnd);
}

fn validate_hotkeys(config: &Config) -> std::result::Result<(), String> {
    hotkey::parse_hotkey(&config.hotkeys.push_to_talk)?;
    hotkey::parse_hotkey(&config.hotkeys.keep_talking)?;
    hotkey::parse_hotkey(&config.hotkeys.streaming)?;
    hotkey::parse_hotkey(&config.hotkeys.resend_selected)?;
    Ok(())
}

unsafe fn update_meter(hwnd: HWND) {
    let selected_device = get_selected_combo_text(hwnd, IDC_AUDIO_INPUT);
    let requested = (selected_device != "Default system input").then_some(selected_device.clone());
    let state = get_window_state(hwnd);

    if state
        .as_ref()
        .and_then(|window| window.meter_device.as_ref())
        != requested.as_ref()
    {
        if let Some(window) = state {
            window.meter = AudioMeter::new(requested.as_deref());
            window.meter_device = requested.clone();
        }
    }

    let label = if let Some(window) = get_window_state(hwnd) {
        match window.meter.as_mut() {
            Some(meter) => format_meter_text(meter.sample_level()),
            None => "Mic activity: unavailable".to_string(),
        }
    } else {
        "Mic activity: unavailable".to_string()
    };

    set_control_text(hwnd, IDC_MIC_ACTIVITY, &label);
}

unsafe fn reset_meter(hwnd: HWND) {
    if let Some(window) = get_window_state(hwnd) {
        window.meter = None;
        window.meter_device = None;
    }
    set_control_text(hwnd, IDC_MIC_ACTIVITY, "Mic activity: reconnecting...");
}

unsafe fn get_window_state(hwnd: HWND) -> Option<&'static mut SettingsWindowState> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindowState;
    (!ptr.is_null()).then_some(&mut *ptr)
}

unsafe fn drop_window_state(hwnd: HWND) {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindowState;
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    }
}

fn format_meter_text(level: u8) -> String {
    let filled = (level / 10).min(10);
    let empty = 10 - filled;
    format!(
        "Mic activity: [{}{}] {}%",
        "|".repeat(filled as usize),
        " ".repeat(empty as usize),
        level
    )
}

fn format_key_terms_display(key_terms: &[String]) -> String {
    key_terms
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_key_terms_text(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_string())
        .collect()
}

unsafe fn create_label(
    hwnd: HWND,
    hinstance: HINSTANCE,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<HWND> {
    create_label_with_id(hwnd, hinstance, 0, text, x, y, w, h)
}

unsafe fn create_label_with_id(
    hwnd: HWND,
    hinstance: HINSTANCE,
    id: i32,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<HWND> {
    let text = to_wide(text);
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        PCWSTR(text.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        x,
        y,
        w,
        h,
        Some(hwnd),
        (id != 0).then_some(HMENU(id as isize as *mut _)),
        Some(hinstance),
        None,
    )
}

unsafe fn create_edit(
    hwnd: HWND,
    hinstance: HINSTANCE,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    password: bool,
) {
    let style = WS_CHILD
        | WS_VISIBLE
        | WS_TABSTOP
        | WINDOW_STYLE(ES_AUTOHSCROLL as u32)
        | if password {
            WINDOW_STYLE(ES_PASSWORD as u32)
        } else {
            WINDOW_STYLE(0)
        };
    let _ = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        style,
        x,
        y,
        w,
        h,
        Some(hwnd),
        Some(HMENU(id as isize as *mut _)),
        Some(hinstance),
        None,
    );
}

unsafe fn create_multiline_edit(
    hwnd: HWND,
    hinstance: HINSTANCE,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let _ = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WS_VSCROLL
            | WINDOW_STYLE(ES_MULTILINE as u32)
            | WINDOW_STYLE(ES_AUTOVSCROLL as u32),
        x,
        y,
        w,
        h,
        Some(hwnd),
        Some(HMENU(id as isize as *mut _)),
        Some(hinstance),
        None,
    );
}

unsafe fn create_checkbox(
    hwnd: HWND,
    hinstance: HINSTANCE,
    id: i32,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let text = to_wide(text);
    let _ = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(text.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        x,
        y,
        w,
        h,
        Some(hwnd),
        Some(HMENU(id as isize as *mut _)),
        Some(hinstance),
        None,
    );
}

unsafe fn create_combo_box(
    hwnd: HWND,
    hinstance: HINSTANCE,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let _ = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("COMBOBOX"),
        w!(""),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        x,
        y,
        w,
        h,
        Some(hwnd),
        Some(HMENU(id as isize as *mut _)),
        Some(hinstance),
        None,
    );
}

unsafe fn set_control_text(hwnd: HWND, id: i32, text: &str) {
    if let Ok(control) = GetDlgItem(Some(hwnd), id) {
        let wide = to_wide(text);
        let _ = SetWindowTextW(control, PCWSTR(wide.as_ptr()));
    }
}

unsafe fn get_control_text(hwnd: HWND, id: i32) -> String {
    if let Ok(control) = GetDlgItem(Some(hwnd), id) {
        let len = GetWindowTextLengthW(control) as usize;
        let mut buf = vec![0u16; len + 1];
        let copied = GetWindowTextW(control, &mut buf);
        return String::from_utf16_lossy(&buf[..copied as usize]);
    }
    String::new()
}

unsafe fn is_checked(hwnd: HWND, id: i32) -> bool {
    GetDlgItem(Some(hwnd), id)
        .ok()
        .map(|control| send_message(control, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == BST_CHECKED as isize)
        .unwrap_or(false)
}

unsafe fn select_combo_value(hwnd: HWND, id: i32, target: &str) {
    let control = match GetDlgItem(Some(hwnd), id) {
        Ok(control) => control,
        Err(_) => return,
    };
    let count = send_message(control, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0 as isize;
    for index in 0..count {
        let text = get_combo_text(control, index as usize);
        if text.eq_ignore_ascii_case(target) {
            send_message(control, CB_SETCURSEL, WPARAM(index as usize), LPARAM(0));
            break;
        }
    }
}

unsafe fn get_selected_combo_text(hwnd: HWND, id: i32) -> String {
    let control = match GetDlgItem(Some(hwnd), id) {
        Ok(control) => control,
        Err(_) => return String::new(),
    };
    let selected = send_message(control, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if selected < 0 {
        String::new()
    } else {
        get_combo_text(control, selected as usize)
    }
}

unsafe fn get_combo_text(control: HWND, index: usize) -> String {
    let len = send_message(control, CB_GETLBTEXTLEN, WPARAM(index), LPARAM(0)).0 as usize;
    let mut buf = vec![0u16; len + 1];
    send_message(
        control,
        CB_GETLBTEXT,
        WPARAM(index),
        LPARAM(buf.as_mut_ptr() as isize),
    );
    String::from_utf16_lossy(&buf[..len])
}

fn send_message(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { SendMessageW(hwnd, message, Some(wparam), Some(lparam)) }
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_key_terms_display_uses_commas() {
        let formatted = format_key_terms_display(&[
            "PowerShell".to_string(),
            " Deepgram ".to_string(),
            String::new(),
        ]);

        assert_eq!(formatted, "PowerShell, Deepgram");
    }

    #[test]
    fn parse_key_terms_text_splits_on_commas() {
        let parsed = parse_key_terms_text("PowerShell, Deepgram , Kubernetes,,");

        assert_eq!(parsed, vec!["PowerShell", "Deepgram", "Kubernetes"]);
    }
}

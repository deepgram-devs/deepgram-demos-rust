use windows::{
    Win32::UI::Input::KeyboardAndMouse::*,
};

/// Types the given text into whatever window currently has focus,
/// using Unicode SendInput so all characters work regardless of layout.
pub fn type_text(text: &str) {
    let inputs: Vec<INPUT> = text
        .encode_utf16()
        .flat_map(|ch| {
            [
                make_unicode_input(ch, 0),
                make_unicode_input(ch, KEYEVENTF_KEYUP.0),
            ]
        })
        .collect();

    if inputs.is_empty() {
        return;
    }

    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn make_unicode_input(ch: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: ch,
                dwFlags: KEYEVENTF_UNICODE | KEYBD_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

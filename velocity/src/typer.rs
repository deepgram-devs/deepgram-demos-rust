use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// Types the given text into whatever window currently has focus,
/// using Unicode SendInput so all characters work regardless of layout.
pub fn type_text(text: &str) -> Result<(), String> {
    let mut inputs = Vec::new();
    for ch in text.chars() {
        match ch {
            '\r' => {}
            '\n' => {
                inputs.push(make_key_input(VK_RETURN, 0));
                inputs.push(make_key_input(VK_RETURN, KEYEVENTF_KEYUP.0));
            }
            _ => {
                let mut utf16 = [0u16; 2];
                for code_unit in ch.encode_utf16(&mut utf16) {
                    inputs.push(make_unicode_input(*code_unit, 0));
                    inputs.push(make_unicode_input(*code_unit, KEYEVENTF_KEYUP.0));
                }
            }
        }
    }

    if inputs.is_empty() {
        return Ok(());
    }

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err("Failed to send transcript keyboard input".to_string());
    }

    Ok(())
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

fn make_key_input(vk: VIRTUAL_KEY, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

pub fn paste_clipboard() -> Result<(), String> {
    let inputs = vec![
        make_key_input(VK_CONTROL, 0),
        make_key_input(VIRTUAL_KEY('V' as u16), 0),
        make_key_input(VIRTUAL_KEY('V' as u16), KEYEVENTF_KEYUP.0),
        make_key_input(VK_CONTROL, KEYEVENTF_KEYUP.0),
    ];

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err("Failed to send paste keyboard input".to_string());
    }

    Ok(())
}

use crate::clipboard;
use crate::config::OutputMode;
use crate::typer;

pub fn deliver_text(text: &str, output_mode: OutputMode, append_newline: bool) -> Result<String, String> {
    let final_text = format_text(text, append_newline);

    match output_mode {
        OutputMode::DirectInput => typer::type_text(&final_text),
        OutputMode::Clipboard => clipboard::copy_text(&final_text)?,
        OutputMode::Paste => {
            clipboard::copy_text(&final_text)?;
            typer::paste_clipboard();
        }
    }

    Ok(final_text)
}

pub fn format_text(text: &str, append_newline: bool) -> String {
    if append_newline {
        format!("{text}\n")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_text_appends_newline_only_when_requested() {
        assert_eq!(format_text("hello", false), "hello");
        assert_eq!(format_text("hello", true), "hello\n");
    }
}

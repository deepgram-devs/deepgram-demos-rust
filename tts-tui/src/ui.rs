use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Table, Row, Cell, Clear, Wrap},
    Frame,
};

use crate::app::{App, Panel, CurrentScreen, CurrentlyEditing};

pub fn render_ui(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(10),
            Constraint::Length(3),
        ])
        .split(size);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // Render Text List Panel
    let text_block_title = if app.focused_panel == Panel::TextList {
        " Saved Texts (Focused) "
    } else {
        " Saved Texts "
    };
    let text_block_style = if app.focused_panel == Panel::TextList {
        Style::default().fg(ratatui::style::Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let text_block = Block::default()
        .borders(Borders::ALL)
        .style(text_block_style)
        .border_type(BorderType::Rounded)
        .title(text_block_title);

    let rows: Vec<Row> = app.saved_texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let is_selected = app.text_table_state.selected() == Some(i);
            let is_loading = app.is_loading && is_selected && app.focused_panel == Panel::TextList;

            let display_text = if is_loading {
                format!("{} {}", app.get_spinner_char(), text)
            } else {
                text.clone()
            };

            let style = if is_selected && app.focused_panel == Panel::TextList {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(display_text)]).style(style)
        })
        .collect();

    let text_table = Table::new(rows, [Constraint::Percentage(100)])
        .block(text_block)
        .header(Row::new(vec!["Text"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(text_table, main_chunks[0], &mut app.text_table_state);

    // Render Voice Menu Panel
    let voice_block_title = if app.focused_panel == Panel::VoiceMenu {
        if app.voice_filter.is_empty() {
            " Deepgram Voices (Focused) ".to_string()
        } else {
            format!(" Deepgram Voices (Focused) - Filter: {} ", app.voice_filter)
        }
    } else {
        if app.voice_filter.is_empty() {
            " Deepgram Voices ".to_string()
        } else {
            format!(" Deepgram Voices - Filter: {} ", app.voice_filter)
        }
    };
    let voice_block_style = if app.focused_panel == Panel::VoiceMenu {
        Style::default().fg(ratatui::style::Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let voice_block = Block::default()
        .borders(Borders::ALL)
        .style(voice_block_style)
        .border_type(BorderType::Rounded)
        .title(voice_block_title);

    let filtered_voices = app.get_filtered_voices();
    let voice_items: Vec<ListItem> = filtered_voices
        .iter()
        .enumerate()
        .map(|(i, voice)| {
            let style = if app.voice_menu_state.selected() == Some(i) && app.focused_panel == Panel::VoiceMenu {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} - {}", voice.name, voice.language)).style(style)
        })
        .collect();

    let voice_list = List::new(voice_items)
        .block(voice_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(voice_list, main_chunks[1], &mut app.voice_menu_state);

    // Render Logs Panel
    let logs_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Logs ");

    let log_items: Vec<ListItem> = app.logs
        .iter()
        .rev()
        .map(|log| ListItem::new(log.clone()))
        .collect();

    let logs_list = List::new(log_items).block(logs_block);
    f.render_widget(logs_list, chunks[1]);

    // Status bar at the bottom
    let status_block = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .title(" Status ");

    let status_line = if app.is_loading {
        Line::from(vec![
            Span::styled(
                format!("{} ", app.get_spinner_char()),
                Style::default().fg(Color::Rgb(19, 239, 147)) // Deepgram green
            ),
            Span::raw(format!("Generating audio... | Speed: {:.2}x", app.playback_speed)),
        ])
    } else {
        Line::from(vec![
            Span::raw(format!("Speed: {:.2}x | {}", app.playback_speed, app.status_message)),
        ])
    };

    let status_text = Paragraph::new(status_line).block(status_block);
    f.render_widget(status_text, chunks[2]);

    // Render Popup for Editing Text
    if let Some(CurrentlyEditing::Text) = &app.currently_editing {
        let popup_block = Block::default()
            .title(" Enter New Text ")
            .borders(Borders::ALL)
            .style(Style::default().bg(ratatui::style::Color::DarkGray));

        let area = centered_rect(60, 20, size);
        f.render_widget(Clear, area); // Clear the area under the popup

        let input_paragraph = Paragraph::new(app.input_buffer.clone())
            .block(popup_block)
            .style(Style::default().fg(ratatui::style::Color::White))
            .wrap(Wrap { trim: false });

        f.render_widget(input_paragraph, area);
    }

    // Render Help Screen
    if app.current_screen == CurrentScreen::Help {
        let help_text = vec![
            "Keyboard Shortcuts:",
            "",
            "Main Screen:",
            "  q         - Quit application",
            "  n         - Add new text",
            "  d         - Delete selected text",
            "  Enter     - Play selected text with selected voice",
            "  Up/Down   - Navigate text list or voice menu",
            "  Left      - Focus previous panel",
            "  Right/Tab - Focus next panel",
            "  +/=       - Increase playback speed",
            "  -         - Decrease playback speed",
            "  0         - Reset speed to 1.0x",
            "  ?         - Show this help screen",
            "",
            "Text Entry Screen:",
            "  Enter     - Save text",
            "  Esc       - Cancel",
            "  Ctrl+V    - Paste from clipboard",
            "  Backspace - Delete character",
            "",
            "Press ESC to close this help screen",
        ];

        let help_block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .style(Style::default().bg(ratatui::style::Color::DarkGray));

        let area = centered_rect(70, 60, size);
        f.render_widget(Clear, area);

        let help_paragraph = Paragraph::new(help_text.join("\n"))
            .block(help_block)
            .style(Style::default().fg(ratatui::style::Color::White));

        f.render_widget(help_paragraph, area);
    }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
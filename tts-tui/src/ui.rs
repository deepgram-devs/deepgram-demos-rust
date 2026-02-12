use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Table, Row, Cell, Clear, Wrap, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::app::{App, Panel, CurrentScreen, CurrentlyEditing, LogLevel, Gender};

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
                format!("{} {} ({} chars)", app.get_spinner_char(), text, text.len())
            } else {
                format!("{} ({} chars)", text, text.len())
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

    // Store bounds for mouse click handling
    app.text_panel_bounds = main_chunks[0];
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
    let voice_items: Vec<ListItem> = {
        let mut items = Vec::new();
        let mut current_language: Option<String> = None;
        let mut item_index = 0;

        for voice in filtered_voices.iter() {
            // Add language separator
            if current_language.as_ref() != Some(&voice.language) {
                current_language = Some(voice.language.clone());
                let separator = format!("━━ {} ━━", voice.language);
                items.push(ListItem::new(separator).style(Style::default().fg(Color::DarkGray)));
                item_index += 1;
            }

            let style = if app.voice_menu_state.selected() == Some(item_index) && app.focused_panel == Panel::VoiceMenu {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let gender_indicator = match voice.gender {
                Gender::Male => "♂",
                Gender::Female => "♀",
            };
            items.push(ListItem::new(format!("  {} {}", voice.name, gender_indicator)).style(style));
            item_index += 1;
        }
        items
    };

    let voice_list = List::new(voice_items)
        .block(voice_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    // Store bounds for mouse click handling
    app.voice_panel_bounds = main_chunks[1];
    f.render_stateful_widget(voice_list, main_chunks[1], &mut app.voice_menu_state);

    // Render Logs Panel
    let logs_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Logs ");

    let log_items: Vec<ListItem> = app.logs
        .iter()
        .rev()
        .map(|(level, message)| {
            let (icon, color) = match level {
                LogLevel::Success => ("✓", Color::Green),
                LogLevel::Error => ("✗", Color::Red),
                LogLevel::Info => ("ℹ", Color::Blue),
            };
            let styled_msg = Span::styled(
                format!("{} {}", icon, message),
                Style::default().fg(color),
            );
            ListItem::new(Line::from(styled_msg))
        })
        .collect();

    let logs_list = List::new(log_items).block(logs_block);
    f.render_widget(logs_list, chunks[1]);

    // Status bar at the bottom
    let status_block = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .title(" Status ");

    let status_line = if app.is_loading {
        let (elapsed, total) = app.get_playback_progress();
        if total > 0 {
            // Show inline progress bar when we have duration
            let progress = (elapsed as f64 / total as f64).clamp(0.0, 1.0);
            let bar_width = 20;
            let filled = (bar_width as f64 * progress) as usize;
            let progress_bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(bar_width - filled));

            Line::from(vec![
                Span::styled(
                    format!("{} ", progress_bar),
                    Style::default().fg(Color::Rgb(19, 239, 147))
                ),
                Span::raw(format!("Speed: {:.2}x | Playing ({:.1}s / {:.1}s) | Press ESC to stop",
                    app.playback_speed, elapsed as f64 / 1000.0, total as f64 / 1000.0)),
            ])
        } else {
            // Show spinner when loading but duration not available yet
            Line::from(vec![
                Span::styled(
                    format!("{} ", app.get_spinner_char()),
                    Style::default().fg(Color::Rgb(19, 239, 147)) // Deepgram green
                ),
                Span::raw(format!("Generating audio... | Speed: {:.2}x", app.playback_speed)),
            ])
        }
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
            "  q         - Quit application (TextList focused)",
            "  Ctrl+Q    - Quit application (from any panel)",
            "  n         - Add new text",
            "  d         - Delete selected text",
            "  Enter     - Play selected text with selected voice",
            "  Up/Down   - Navigate text list or voice menu",
            "  Left      - Focus previous panel",
            "  Right/Tab - Focus next panel",
            "  +/=       - Increase playback speed",
            "  -         - Decrease playback speed",
            "  0         - Reset speed to 1.0x",
            "  Esc       - Stop audio playback or clear voice filter",
            "  ?         - Show this help screen",
            "",
            "Text Entry Screen:",
            "  Enter     - Save text",
            "  Esc       - Cancel",
            "  Ctrl+V    - Paste from clipboard",
            "  Backspace - Delete character",
            "",
            "Help Screen:",
            "  Up/Down   - Scroll help text",
            "  Esc       - Close this help screen",
        ];

        let help_block = Block::default()
            .title(" Help (scroll with Up/Down) ")
            .borders(Borders::ALL)
            .style(Style::default().bg(ratatui::style::Color::DarkGray));

        let area = centered_rect(70, 60, size);
        f.render_widget(Clear, area);

        let visible_lines = (area.height as usize).saturating_sub(2); // Account for borders

        // Get the slice of help lines to display based on scroll offset
        let start = app.help_scroll_offset;
        let end = (start + visible_lines).min(help_text.len());
        let displayed_lines: Vec<&str> = help_text[start..end].to_vec();

        let help_paragraph = Paragraph::new(displayed_lines.join("\n"))
            .block(help_block)
            .style(Style::default().fg(ratatui::style::Color::White));

        f.render_widget(help_paragraph, area);

        // Render scrollbar if needed
        if help_text.len() > visible_lines {
            let mut scrollbar_state = ratatui::widgets::ScrollbarState::default()
                .content_length(help_text.len())
                .position(app.help_scroll_offset);

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight);

            f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
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
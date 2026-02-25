use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Table, Row, Cell, Clear, Wrap, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::app::{App, Panel, CurrentScreen, LogLevel, LogEntry, Gender, AUDIO_FORMATS, DEFAULT_FORMAT_INDEX};
use crate::theme::THEMES;

pub fn render_ui(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let theme = app.current_theme();
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
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(chunks[0]);

    // Render Text List Panel
    let text_block_title = if app.text_filter.is_empty() {
        " Saved Texts ".to_string()
    } else {
        format!(" Saved Texts — Filter: {} ", app.text_filter)
    };
    let text_block_style = if app.focused_panel == Panel::TextList {
        Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let text_block = Block::default()
        .borders(Borders::ALL)
        .style(text_block_style)
        .border_type(BorderType::Rounded)
        .title(text_block_title);

    let filtered_texts = app.get_filtered_texts();
    let rows: Vec<Row> = filtered_texts
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
                let dist = app.text_table_state.selected()
                    .map(|sel| (i as isize - sel as isize).unsigned_abs())
                    .unwrap_or(0);
                Style::default().fg(fade_color(theme.text_list_near, theme.text_list_far, dist, 12))
            };
            Row::new(vec![Cell::from(display_text)]).style(style)
        })
        .collect();

    let text_table = Table::new(rows, [Constraint::Percentage(100)])
        .block(text_block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    // Store bounds for mouse click handling
    app.text_panel_bounds = main_chunks[0];
    f.render_stateful_widget(text_table, main_chunks[0], &mut app.text_table_state);

    // Render Voice Menu Panel
    let voice_block_title = if app.voice_filter.is_empty() {
        " Deepgram Voices ".to_string()
    } else {
        format!(" Deepgram Voices — Filter: {} ", app.voice_filter)
    };
    let voice_block_style = if app.focused_panel == Panel::VoiceMenu {
        Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)
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
                let dist = app.voice_menu_state.selected()
                    .map(|sel| (item_index as isize - sel as isize).unsigned_abs())
                    .unwrap_or(0);
                Style::default().fg(fade_color(theme.voice_list_near, theme.voice_list_far, dist, 12))
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
    let logs_area = chunks[1];
    app.log_panel_bounds = logs_area;

    // Build all rendered lines newest-first
    let all_log_lines: Vec<Line> = app.logs
        .iter()
        .rev()
        .flat_map(|entry: &LogEntry| {
            let (icon, color) = match entry.level {
                LogLevel::Success => ("✓", theme.success),
                LogLevel::Error   => ("✗", theme.error),
                LogLevel::Warning => ("⚠", theme.warning),
                LogLevel::Info    => ("ℹ", theme.secondary),
            };
            let ts = entry.timestamp.format("%H:%M:%S").to_string();
            let message_lines: Vec<String> = entry.message.lines().map(|s| s.to_string()).collect();
            message_lines.into_iter().enumerate().map(move |(i, line)| {
                if i == 0 {
                    Line::from(vec![
                        Span::styled(format!("{} {} ", ts, icon), Style::default().fg(Color::DarkGray)),
                        Span::styled(line, Style::default().fg(color)),
                    ])
                } else {
                    Line::from(Span::styled(format!("         {}", line), Style::default().fg(color)))
                }
            }).collect::<Vec<Line>>()
        })
        .collect();

    // Clamp scroll offset so we can't scroll past the oldest entry
    let visible_rows = logs_area.height.saturating_sub(2) as usize; // subtract borders
    let max_offset = all_log_lines.len().saturating_sub(visible_rows);
    let scroll_offset = app.log_scroll_offset.min(max_offset);
    app.log_scroll_offset = scroll_offset;

    let logs_title = if scroll_offset > 0 {
        format!(" Logs (scrolled +{}) ", scroll_offset)
    } else {
        " Logs ".to_string()
    };

    let logs_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(logs_title);

    let logs_paragraph = Paragraph::new(all_log_lines.clone())
        .block(logs_block)
        .scroll((scroll_offset as u16, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(logs_paragraph, logs_area);

    // Scrollbar for logs
    if all_log_lines.len() > visible_rows {
        let mut scrollbar_state = ratatui::widgets::ScrollbarState::default()
            .content_length(all_log_lines.len())
            .position(scroll_offset);
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight);
        f.render_stateful_widget(scrollbar, logs_area, &mut scrollbar_state);
    }

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
                    Style::default().fg(theme.primary)
                ),
                Span::raw(format!("Speed: {:.2}x | {} | {} Hz | Playing ({:.1}s / {:.1}s) | Press ESC to stop",
                    app.playback_speed, app.current_audio_format().display_name, app.sample_rate,
                    elapsed as f64 / 1000.0, total as f64 / 1000.0)),
            ])
        } else {
            // Show spinner when loading but duration not available yet
            Line::from(vec![
                Span::styled(
                    format!("{} ", app.get_spinner_char()),
                    Style::default().fg(theme.primary)
                ),
                Span::raw(format!("Generating audio... | Speed: {:.2}x | {} | {} Hz",
                    app.playback_speed, app.current_audio_format().display_name, app.sample_rate)),
            ])
        }
    } else {
        Line::from(vec![
            Span::raw(format!("Speed: {:.2}x | {} | {} Hz | {}",
                app.playback_speed, app.current_audio_format().display_name,
                app.sample_rate, app.status_message)),
        ])
    };

    let status_text = Paragraph::new(status_line).block(status_block);
    f.render_widget(status_text, chunks[2]);

    // Render Popup for Editing Text
    if app.current_screen == CurrentScreen::Editing {
        let popup_block = Block::default()
            .title(" Enter New Text ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.primary))
            .style(Style::default().bg(Color::DarkGray));

        let area = centered_rect(60, 20, size);
        f.render_widget(Clear, area); // Clear the area under the popup

        let input_paragraph = Paragraph::new(app.input_buffer.clone())
            .block(popup_block)
            .style(Style::default().fg(theme.primary_light))
            .wrap(Wrap { trim: false });

        f.render_widget(input_paragraph, area);
    }

    // Render Voice Filter Popup
    if app.current_screen == CurrentScreen::VoiceFilter {
        let area = centered_rect(50, 20, size);
        f.render_widget(Clear, area);

        let hint = if app.voice_filter_buffer.is_empty() {
            " Filter Voices — Enter to apply, Esc to cancel ".to_string()
        } else {
            format!(" Filter Voices — {} match(es) ", app.get_filtered_voices_for_buffer().len())
        };

        let popup_block = Block::default()
            .title(hint)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(Color::DarkGray))
            .border_style(Style::default().fg(theme.secondary));

        // Show buffer with a blinking-cursor indicator
        let display = format!("{}_", app.voice_filter_buffer);
        let input_paragraph = Paragraph::new(display)
            .block(popup_block)
            .style(Style::default().fg(theme.secondary_light))
            .wrap(Wrap { trim: false });

        f.render_widget(input_paragraph, area);

        // Hint line below popup
        let hint_area = Rect {
            x: area.x,
            y: area.y + area.height,
            width: area.width,
            height: 1,
        };
        if hint_area.y < size.height {
            let shortcuts = Paragraph::new(Line::from(vec![
                Span::styled(" Enter", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
                Span::raw(" apply  "),
                Span::styled("Esc", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                Span::raw(" cancel  "),
                Span::styled("Ctrl+U", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                Span::raw(" clear "),
            ]));
            f.render_widget(shortcuts, hint_area);
        }
    }

    // Render Text Filter Popup
    if app.current_screen == CurrentScreen::TextFilter {
        let area = centered_rect(50, 20, size);
        f.render_widget(Clear, area);

        let hint = if app.text_filter_buffer.is_empty() {
            " Filter Texts — Enter to apply, Esc to cancel ".to_string()
        } else {
            format!(" Filter Texts — {} match(es) ", app.get_filtered_texts_for_buffer().len())
        };

        let popup_block = Block::default()
            .title(hint)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(Color::DarkGray))
            .border_style(Style::default().fg(theme.primary));

        let display = format!("{}_", app.text_filter_buffer);
        let input_paragraph = Paragraph::new(display)
            .block(popup_block)
            .style(Style::default().fg(theme.primary_light))
            .wrap(Wrap { trim: false });

        f.render_widget(input_paragraph, area);

        let hint_area = Rect {
            x: area.x,
            y: area.y + area.height,
            width: area.width,
            height: 1,
        };
        if hint_area.y < size.height {
            let shortcuts = Paragraph::new(Line::from(vec![
                Span::styled(" Enter", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
                Span::raw(" apply  "),
                Span::styled("Esc", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                Span::raw(" cancel  "),
                Span::styled("Ctrl+U", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                Span::raw(" clear "),
            ]));
            f.render_widget(shortcuts, hint_area);
        }
    }

    // Render Popup for API Key Input
    if app.current_screen == CurrentScreen::ApiKeyInput {
        let title = Line::from(vec![
            Span::styled(" Set Deepgram API Key ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::styled("(input hidden) ", Style::default().fg(Color::DarkGray)),
        ]);

        let popup_block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.warning))
            .style(Style::default().bg(Color::DarkGray));

        let area = centered_rect_fixed(60, 5, size);
        f.render_widget(Clear, area);

        // Mask the key input for security
        let char_count = app.api_key_input_buffer.len();
        let masked = if char_count == 0 {
            Span::styled("Enter API key…", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(
                format!("{}_", "*".repeat(char_count)),
                Style::default().fg(theme.warning),
            )
        };
        let input_paragraph = Paragraph::new(Line::from(masked))
            .block(popup_block)
            .wrap(Wrap { trim: false });

        f.render_widget(input_paragraph, area);

        // Hint row below popup
        let hint_area = Rect { x: area.x, y: area.y + area.height, width: area.width, height: 1 };
        if hint_area.y < size.height {
            let hints = Paragraph::new(Line::from(vec![
                Span::styled(" Enter", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
                Span::raw(" save  "),
                Span::styled("Esc", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                Span::raw(" cancel "),
            ]));
            f.render_widget(hints, hint_area);
        }
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
            "  k         - Set Deepgram API key interactively",
            "  o         - Open audio cache folder in Finder",
            "  Enter     - Play selected text with selected voice",
            "  Up/Down   - Navigate text list or voice menu",
            "  Left      - Focus previous panel",
            "  Right/Tab - Focus next panel",
            "  +/=       - Increase playback speed",
            "  -         - Decrease playback speed",
            "  0         - Reset speed to 1.0x",
            "  f         - Select audio encoding format",
            "  s         - Select TTS sample rate",
            "  Esc       - Stop audio / clear filter for focused panel",
            "  /         - Open filter popup for focused panel",
            "  t         - Select color theme",
            "  ?         - Show this help screen",
            "",
            "Voice Filter Popup:",
            "  Type      - Narrow voice list (name, language, model)",
            "  Enter     - Apply filter and close",
            "  Esc       - Cancel (keeps previous filter)",
            "  Ctrl+U    - Clear filter text",
            "",
            "Text Filter Popup:",
            "  Type      - Narrow text list by content",
            "  Enter     - Apply filter and close",
            "  Esc       - Cancel (keeps previous filter)",
            "  Ctrl+U    - Clear filter text",
            "",
            "Text Entry Screen:",
            "  Enter     - Save text",
            "  Esc       - Cancel",
            "  Ctrl+V    - Paste from clipboard",
            "  Backspace - Delete character",
            "",
            "API Key Screen:",
            "  Enter     - Save API key",
            "  Esc       - Cancel",
            "  Backspace - Delete character",
            "",
            "Theme Select Popup:",
            "  Up/Down   - Navigate themes",
            "  Enter     - Apply theme and close",
            "  Esc       - Cancel",
            "",
            "Help Screen:",
            "  Up/Down   - Scroll help text",
            "  Esc       - Close this help screen",
        ];

        let help_block = Block::default()
            .title(" Help (scroll with Up/Down) ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.primary))
            .style(Style::default().bg(Color::DarkGray));

        let area = centered_rect(70, 60, size);
        f.render_widget(Clear, area);

        let visible_lines = (area.height as usize).saturating_sub(2); // Account for borders

        // Get the slice of help lines to display based on scroll offset
        let start = app.help_scroll_offset;
        let end = (start + visible_lines).min(help_text.len());
        let displayed_lines: Vec<&str> = help_text[start..end].to_vec();

        let help_paragraph = Paragraph::new(displayed_lines.join("\n"))
            .block(help_block)
            .style(Style::default().fg(theme.primary_light));

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

    // ── Sample Rate Select Popup ──────────────────────────────────────────────
    if app.current_screen == CurrentScreen::SampleRateSelect {
        let fmt = app.current_audio_format();
        let popup_height = fmt.valid_sample_rates.len() as u16 + 4; // items + border + hint
        let area = centered_rect_fixed(44, popup_height, size);
        f.render_widget(Clear, area);

        let items: Vec<ListItem> = fmt.valid_sample_rates.iter().enumerate().map(|(i, &rate)| {
            let is_selected = app.sample_rate_menu_state.selected() == Some(i);
            let is_default = rate == fmt.default_sample_rate;
            let label = if is_default {
                format!("{} Hz (default)", rate)
            } else {
                format!("{} Hz", rate)
            };
            let style = if is_selected {
                Style::default().fg(theme.quaternary).add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(theme.secondary_light)
            };
            ListItem::new(label).style(style)
        }).collect();

        let popup_block = Block::default()
            .title(format!(" Sample Rate — {} ", fmt.display_name))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.quaternary));

        let inner = popup_block.inner(area);
        f.render_widget(popup_block, area);

        // Split inner: list + hint row
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let list = List::new(items);
        f.render_stateful_widget(list, chunks[0], &mut app.sample_rate_menu_state);

        let hint = Line::from(vec![
            Span::styled(" Enter", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
            Span::raw(" apply  "),
            Span::styled("Esc", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ]);
        f.render_widget(Paragraph::new(hint), chunks[1]);
    }

    // ── Audio Format Select Popup ─────────────────────────────────────────────
    if app.current_screen == CurrentScreen::AudioFormatSelect {
        let popup_height = AUDIO_FORMATS.len() as u16 + 4; // items + border + hint
        let area = centered_rect_fixed(54, popup_height, size);
        f.render_widget(Clear, area);

        let items: Vec<ListItem> = AUDIO_FORMATS.iter().enumerate().map(|(i, fmt)| {
            let is_selected = app.audio_format_menu_state.selected() == Some(i);
            let is_default = i == DEFAULT_FORMAT_INDEX;
            // Summarise valid sample rates for the format
            let rates = fmt.valid_sample_rates;
            let rate_summary = if rates.len() == 1 {
                format!("{} Hz", rates[0])
            } else {
                format!("{}–{} kHz", rates[0] / 1000, rates[rates.len() - 1] / 1000)
            };
            let label = if is_default {
                format!("{:<16} {}  (default)", fmt.display_name, rate_summary)
            } else {
                format!("{:<16} {}", fmt.display_name, rate_summary)
            };
            let style = if is_selected {
                Style::default().fg(theme.tertiary).add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(theme.primary_light)
            };
            ListItem::new(label).style(style)
        }).collect();

        let popup_block = Block::default()
            .title(" Audio Format ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.tertiary));

        let inner = popup_block.inner(area);
        f.render_widget(popup_block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let list = List::new(items);
        f.render_stateful_widget(list, chunks[0], &mut app.audio_format_menu_state);

        let hint = Line::from(vec![
            Span::styled(" Enter", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
            Span::raw(" apply  "),
            Span::styled("Esc", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ]);
        f.render_widget(Paragraph::new(hint), chunks[1]);
    }

    // ── Theme Select Popup ────────────────────────────────────────────────────
    if app.current_screen == CurrentScreen::ThemeSelect {
        let popup_height = THEMES.len() as u16 * 2 + 4; // 2 rows per theme + border + hint
        let area = centered_rect_fixed(54, popup_height, size);
        f.render_widget(Clear, area);

        // Build one item per theme: name in its own primary color, description dimmed below
        let items: Vec<ListItem> = THEMES.iter().enumerate().map(|(i, t)| {
            let is_selected = app.theme_menu_state.selected() == Some(i);
            let is_current  = i == app.theme_index;
            let marker = if is_current { "● " } else { "  " };

            let name_style = if is_selected {
                Style::default().fg(t.primary).add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(t.primary).add_modifier(Modifier::BOLD)
            };
            let desc_style = Style::default().fg(Color::DarkGray);

            let name_line = Line::from(vec![
                Span::styled(marker, name_style),
                Span::styled(t.name, name_style),
            ]);
            let desc_line = Line::from(Span::styled(
                format!("    {}", t.description),
                desc_style,
            ));
            ListItem::new(vec![name_line, desc_line])
        }).collect();

        let popup_block = Block::default()
            .title(" Select Theme ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(Color::DarkGray))
            .border_style(Style::default().fg(theme.primary));

        let inner = popup_block.inner(area);
        f.render_widget(popup_block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let list = List::new(items);
        f.render_stateful_widget(list, chunks[0], &mut app.theme_menu_state);

        let hint = Line::from(vec![
            Span::styled(" Enter", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
            Span::raw(" apply  "),
            Span::styled("Esc", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel  "),
            Span::styled("Up/Down", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
            Span::raw(" navigate"),
        ]);
        f.render_widget(Paragraph::new(hint), chunks[1]);
    }
}

/// Helper that centers a rect with a percentage width but a fixed row height.
fn centered_rect_fixed(percent_x: u16, height: u16, r: Rect) -> Rect {
    let top_pad = r.height.saturating_sub(height) / 2;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_pad),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Interpolates between `near` (distance 0) and `far` (distance >= `max_dist`).
/// Used to make list items progressively dimmer the further they are from the selection.
fn fade_color(near: (u8, u8, u8), far: (u8, u8, u8), distance: usize, max_dist: usize) -> Color {
    let t = (distance as f32 / max_dist as f32).min(1.0);
    Color::Rgb(
        (near.0 as f32 + (far.0 as f32 - near.0 as f32) * t) as u8,
        (near.1 as f32 + (far.1 as f32 - near.1 as f32) * t) as u8,
        (near.2 as f32 + (far.2 as f32 - near.2 as f32) * t) as u8,
    )
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
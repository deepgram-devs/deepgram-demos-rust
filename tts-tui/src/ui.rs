use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Table, Row, Cell, Clear},
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
            let style = if app.text_table_state.selected() == Some(i) && app.focused_panel == Panel::TextList {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(text.clone())]).style(style)
        })
        .collect();

    let text_table = Table::new(rows, [Constraint::Percentage(100)])
        .block(text_block)
        .header(Row::new(vec!["Text"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(text_table, main_chunks[0], &mut app.text_table_state);

    // Render Voice Menu Panel
    let voice_block_title = if app.focused_panel == Panel::VoiceMenu {
        " Deepgram Voices (Focused) "
    } else {
        " Deepgram Voices "
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

    let voice_items: Vec<ListItem> = app
        .voices
        .iter()
        .enumerate()
        .map(|(i, voice)| {
            let style = if app.voice_menu_state.selected() == Some(i) && app.focused_panel == Panel::VoiceMenu {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} - {}", voice.name, voice.vendor)).style(style)
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

    let status_text = Paragraph::new(app.status_message.clone()).block(status_block);
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
            .style(Style::default().fg(ratatui::style::Color::White));
            
        f.render_widget(input_paragraph, area);
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
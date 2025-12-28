use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::{App, AppState, FocusedPane};

pub fn render(f: &mut Frame, app: &App) {
    match &app.state {
        AppState::TopicInput => render_topic_input(f, app),
        AppState::SpeakerCountSelection => render_speaker_count_selection(f, app),
        AppState::VoiceAssignment => render_voice_assignment(f, app),
        AppState::GeneratingPodcast => render_generating(f, app),
        AppState::Completed => render_completed(f, app),
        AppState::Error(e) => render_error(f, e),
    }
}

fn render_topic_input(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let title = Paragraph::new("Podcast Generator")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let input = Paragraph::new(app.topic.as_str())
        .style(Style::default().fg(Color::White))
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Enter podcast topic (CMD+V or Ctrl+V to paste, Enter to continue, Esc to quit)"));
    f.render_widget(input, chunks[1]);

    let help_text = vec![
        Line::from(""),
        Line::from("Enter a topic for your podcast."),
        Line::from("You can paste text using CMD+V (macOS) or Ctrl+V (Linux/Windows)."),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  - The history of nuclear power plants"),
        Line::from("  - The invention of AI software and hardware"),
        Line::from("  - 120-volt vs 240-volt infrastructure worldwide"),
    ];
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, chunks[2]);
}

fn render_speaker_count_selection(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(0),
        ])
        .split(area);

    let title = Paragraph::new(format!("Topic: {}", app.topic))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let options = vec![
        Line::from(""),
        Line::from(format!("  [1] One Speaker   {}", if app.speaker_count == 1 { "<-" } else { "" })),
        Line::from(format!("  [2] Two Speakers  {}", if app.speaker_count == 2 { "<-" } else { "" })),
        Line::from(format!("  [3] Three Speakers {}", if app.speaker_count == 3 { "<-" } else { "" })),
        Line::from(format!("  [4] Four Speakers {}", if app.speaker_count == 4 { "<-" } else { "" })),
    ];
    let selection = Paragraph::new(options)
        .style(Style::default().fg(Color::White))
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Select number of speakers (Press 1-4, then Enter. Esc to go back)"));
    f.render_widget(selection, chunks[1]);
}

fn render_voice_assignment(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new(format!("Topic: {} | Speakers: {}", app.topic, app.speaker_count))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    render_speaker_list(f, app, main_chunks[0]);
    render_voice_list(f, app, main_chunks[1]);

    let all_assigned = app.speakers.iter().all(|s| s.voice_name.is_some());
    let help_text = if all_assigned {
        "Press Ctrl+G to generate podcast | Arrow keys to navigate | Enter to assign | Esc to go back"
    } else {
        "Arrow keys to navigate between panes and items | Enter to assign voice | Esc to go back"
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(if all_assigned { Color::Green } else { Color::Yellow }))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

fn render_speaker_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.speakers
        .iter()
        .enumerate()
        .map(|(i, speaker)| {
            let voice_name = speaker.voice_name.as_deref().unwrap_or("Not assigned");
            let content = format!("Speaker {} -> {}", speaker.speaker_id, voice_name);

            let style = if i == app.selected_speaker && app.focused_pane == FocusedPane::Left {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if i == app.selected_speaker {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let border_style = if app.focused_pane == FocusedPane::Left {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Speakers")
            .border_style(border_style));

    f.render_widget(list, area);
}

fn render_voice_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.available_voices
        .iter()
        .map(|voice| {
            ListItem::new(voice.as_str())
        })
        .collect();

    let border_style = if app.focused_pane == FocusedPane::Right {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Deepgram Aura-2 Voices (Press Enter to assign)")
            .border_style(border_style))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol(">> ");

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(app.selected_voice));

    let visible_height = area.height.saturating_sub(2) as usize;
    let offset = if app.selected_voice >= visible_height {
        app.selected_voice.saturating_sub(visible_height - 1)
    } else {
        0
    };

    *list_state.offset_mut() = offset;

    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_generating(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let title = Paragraph::new("Generating Podcast...")
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let progress = Paragraph::new(app.generation_progress.as_str())
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Progress"));
    f.render_widget(progress, chunks[1]);
}

fn render_completed(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Podcast Generated Successfully!")
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let message = Paragraph::new(app.generation_progress.as_str())
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Result"));
    f.render_widget(message, chunks[1]);

    let help = Paragraph::new("Press 'q' to quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

fn render_error(f: &mut Frame, error: &str) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Error")
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let message = Paragraph::new(error)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Error Details"));
    f.render_widget(message, chunks[1]);

    let help = Paragraph::new("Press 'q' to quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

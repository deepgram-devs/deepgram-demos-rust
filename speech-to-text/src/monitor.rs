use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use std::io::{self, Write, stdout};
use std::time::Duration;
use tokio::sync::mpsc;

const RECENT_WORD_COUNT: usize = 10;

#[derive(Clone, Debug)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Streaming,
    Closed,
    Error(String),
}

impl ConnectionStatus {
    fn label(&self) -> String {
        match self {
            Self::Connecting => "connecting".to_string(),
            Self::Connected => "connected".to_string(),
            Self::Streaming => "streaming".to_string(),
            Self::Closed => "closed".to_string(),
            Self::Error(message) => format!("error: {message}"),
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Connecting => Color::Blue,
            Self::Connected => Color::Green,
            Self::Streaming => Color::Cyan,
            Self::Closed => Color::DarkGrey,
            Self::Error(_) => Color::Red,
        }
    }
}

#[derive(Clone, Debug)]
pub enum MonitorEvent {
    Registered {
        connection_id: usize,
    },
    Status {
        connection_id: usize,
        status: ConnectionStatus,
    },
    Connected {
        connection_id: usize,
        request_id: String,
        established_after: Duration,
    },
    Transcript {
        connection_id: usize,
        words: Vec<String>,
        is_final: bool,
    },
}

#[derive(Clone, Debug)]
pub enum MonitorCommand {
    TerminateConnection { connection_id: usize },
    StopAll,
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        let mut out = stdout();
        enable_raw_mode()?;
        execute!(
            out,
            EnterAlternateScreen,
            cursor::Hide,
            Clear(ClearType::All)
        )?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        let _ = execute!(
            out,
            ResetColor,
            cursor::Show,
            Clear(ClearType::All),
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
    }
}

#[derive(Clone, Debug)]
struct ConnectionRow {
    request_id_suffix: Option<String>,
    established_at: Option<Duration>,
    status: ConnectionStatus,
    last_words: Vec<String>,
}

impl Default for ConnectionRow {
    fn default() -> Self {
        Self {
            request_id_suffix: None,
            established_at: None,
            status: ConnectionStatus::Connecting,
            last_words: Vec::new(),
        }
    }
}

pub struct ConnectionMonitor {
    rows: Vec<ConnectionRow>,
    selected_index: usize,
    command_tx: mpsc::UnboundedSender<MonitorCommand>,
}

impl ConnectionMonitor {
    pub fn new(connection_count: usize, command_tx: mpsc::UnboundedSender<MonitorCommand>) -> Self {
        Self {
            rows: vec![ConnectionRow::default(); connection_count],
            selected_index: 0,
            command_tx,
        }
    }

    pub async fn run(mut self, mut rx: mpsc::UnboundedReceiver<MonitorEvent>) -> io::Result<()> {
        let _guard = TerminalGuard::enter()?;
        self.render()?;

        let mut tick = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                event = rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    self.apply(event);
                    self.render()?;
                }
                _ = tick.tick() => {
                    self.handle_input()?;
                    self.render()?;
                }
            }
        }

        Ok(())
    }

    fn apply(&mut self, event: MonitorEvent) {
        match event {
            MonitorEvent::Registered { connection_id } => {
                self.ensure_row(connection_id);
            }
            MonitorEvent::Status {
                connection_id,
                status,
            } => {
                self.ensure_row(connection_id);
                if let Some(row) = self.row_mut(connection_id) {
                    row.status = status;
                }
            }
            MonitorEvent::Connected {
                connection_id,
                request_id,
                established_after,
            } => {
                self.ensure_row(connection_id);
                if let Some(row) = self.row_mut(connection_id) {
                    row.request_id_suffix = Some(request_id_suffix(&request_id));
                    row.established_at = Some(established_after);
                    row.status = ConnectionStatus::Connected;
                }
            }
            MonitorEvent::Transcript {
                connection_id,
                mut words,
                is_final,
            } => {
                self.ensure_row(connection_id);
                if let Some(row) = self.row_mut(connection_id) {
                    if is_final {
                        row.last_words.append(&mut words);
                        if row.last_words.len() > RECENT_WORD_COUNT {
                            row.last_words = row
                                .last_words
                                .split_off(row.last_words.len() - RECENT_WORD_COUNT);
                        }
                    } else {
                        if words.len() > RECENT_WORD_COUNT {
                            words = words.split_off(words.len() - RECENT_WORD_COUNT);
                        }
                        row.last_words = words;
                    }
                }
            }
        }
    }

    fn ensure_row(&mut self, connection_id: usize) {
        while self.rows.len() < connection_id {
            self.rows.push(ConnectionRow::default());
        }
    }

    fn row_mut(&mut self, connection_id: usize) -> Option<&mut ConnectionRow> {
        connection_id
            .checked_sub(1)
            .and_then(|idx| self.rows.get_mut(idx))
    }

    fn handle_input(&mut self) -> io::Result<()> {
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up => {
                        self.selected_index = self.selected_index.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        if self.selected_index + 1 < self.rows.len() {
                            self.selected_index += 1;
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let _ = self.command_tx.send(MonitorCommand::StopAll);
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        if self.selected_index < self.rows.len() {
                            let _ = self.command_tx.send(MonitorCommand::TerminateConnection {
                                connection_id: self.selected_index + 1,
                            });
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                        let _ = self.command_tx.send(MonitorCommand::StopAll);
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn render(&self) -> io::Result<()> {
        let mut out = stdout();
        let (terminal_width, terminal_height) = terminal::size().unwrap_or((100, 24));
        let width = terminal_width as usize;
        let request_width = 14;
        let established_width = 11;
        let status_width = 14;
        let fixed_width = request_width + established_width + status_width + 6;
        let words_width = width.saturating_sub(fixed_width).max(1);
        let mut y = 0;

        execute!(
            out,
            cursor::MoveTo(0, 0),
            Clear(ClearType::All),
            SetForegroundColor(Color::Cyan),
            Print(line_fit("Deepgram streaming connection monitor", width)),
            Clear(ClearType::UntilNewLine),
            ResetColor,
        )?;
        y += 1;

        execute!(
            out,
            cursor::MoveTo(0, y),
            Print(line_fit(
                "Up/Down select  d terminate selected  q/Esc/Ctrl+C stop all",
                width
            )),
            Clear(ClearType::UntilNewLine),
        )?;
        y += 2;

        execute!(
            out,
            cursor::MoveTo(0, y),
            Print(table_line(
                "Request",
                "Established",
                "Status",
                "Last 10 words",
                words_width,
            )),
            Clear(ClearType::UntilNewLine),
        )?;
        y += 1;

        execute!(
            out,
            cursor::MoveTo(0, y),
            Print(table_line(
                &"-".repeat(request_width),
                &"-".repeat(established_width),
                &"-".repeat(status_width),
                &"-".repeat(words_width),
                words_width,
            )),
            Clear(ClearType::UntilNewLine),
        )?;
        y += 1;

        for (idx, row) in self.rows.iter().enumerate() {
            if y >= terminal_height {
                break;
            }

            let request = row
                .request_id_suffix
                .clone()
                .unwrap_or_else(|| format!("pending-{}", idx + 1));
            let established = row
                .established_at
                .map(format_mm_ss)
                .unwrap_or_else(|| "--:--".to_string());
            let status = truncate(&row.status.label(), 14);
            let words = truncate(&row.last_words.join(" "), words_width);
            let selected = idx == self.selected_index;

            if selected {
                execute!(out, SetAttribute(Attribute::Reverse))?;
            }

            execute!(
                out,
                cursor::MoveTo(0, y),
                Print(format_column(&request, request_width)),
                Print("  "),
                Print(format_column(&established, established_width)),
                Print("  "),
                SetForegroundColor(row.status.color()),
                Print(format_column(&status, status_width)),
                ResetColor,
                Print("  "),
                Print(format_column(&words, words_width)),
                Clear(ClearType::UntilNewLine),
            )?;

            if selected {
                execute!(out, SetAttribute(Attribute::NoReverse))?;
            }

            y += 1;
        }

        out.flush()
    }
}

pub fn request_id_suffix(request_id: &str) -> String {
    request_id
        .rsplit_once('-')
        .map(|(_, suffix)| suffix)
        .unwrap_or(request_id)
        .to_string()
}

fn format_mm_ss(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn truncate(value: &str, max_width: usize) -> String {
    if value.chars().count() <= max_width {
        return value.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut truncated = value.chars().take(max_width - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn format_column(value: &str, width: usize) -> String {
    let truncated = truncate(value, width);
    format!("{truncated:<width$}")
}

fn table_line(
    request: &str,
    established: &str,
    status: &str,
    words: &str,
    words_width: usize,
) -> String {
    format!(
        "{}  {}  {}  {}",
        format_column(request, 14),
        format_column(established, 11),
        format_column(status, 14),
        format_column(words, words_width),
    )
}

fn line_fit(value: &str, width: usize) -> String {
    truncate(value, width)
}

#[cfg(test)]
mod tests {
    use super::{format_column, request_id_suffix, table_line};

    #[test]
    fn uses_uuid_suffix_after_final_dash() {
        assert_eq!(
            request_id_suffix("fd2790cb-9de9-4207-93ea-4349d1b74867"),
            "4349d1b74867"
        );
    }

    #[test]
    fn leaves_values_without_dash_intact() {
        assert_eq!(request_id_suffix("abc"), "abc");
    }

    #[test]
    fn table_line_starts_with_request_field() {
        let line = table_line("4349d1b74867", "00:02", "streaming", "hello world", 20);
        assert!(line.starts_with("4349d1b74867"));
    }

    #[test]
    fn column_values_do_not_exceed_width() {
        assert_eq!(
            format_column("abcdefghijklmnopqrstuvwxyz", 8)
                .chars()
                .count(),
            8
        );
    }
}

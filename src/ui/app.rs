//! ## TUI Application / TUI-приложение

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use undertow_protocol::protocol::peer_id::PeerId;

/// События, которые сетевой слой передает в UI
pub enum UiEvent {
    MessageReceived(PeerId, String),
    SystemMessage(String),
    Error(String),
    PeerDiscovered(PeerId, String),
}

/// Структура сообщения чата
pub struct ChatMessage {
    pub sender: PeerId,
    pub text: String,
    pub time: String,
    pub is_system: bool,
}

/// Application state / Состояние приложения
pub struct App {
    pub my_peer_id: PeerId,
    pub username: String,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub input_mode: InputMode,
    pub scroll: usize,
    pub known_peers: Vec<PeerId>,
    pub status: String,
    pub show_help: bool,
    /// Маппинг username → PeerId для отправки по имени
    pub username_to_peer: HashMap<String, PeerId>,
    /// Обратный маппинг PeerId → username для отображения
    pub peer_to_username: HashMap<PeerId, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

impl App {
    pub fn new(my_peer_id: PeerId, username: String) -> Self {
        Self {
            my_peer_id,
            username,
            messages: Vec::new(),
            input: String::new(),
            input_mode: InputMode::Normal,
            scroll: 0,
            known_peers: Vec::new(),
            status: "Ready (Press 'e' to write, 'h' for help)".to_string(),
            show_help: false,
            username_to_peer: HashMap::new(),
            peer_to_username: HashMap::new(),
        }
    }

    pub fn add_message(&mut self, sender: PeerId, text: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages.push(ChatMessage {
            sender,
            text,
            time,
            is_system: false,
        });
        if self.messages.len() > 500 {
            self.messages.remove(0);
        }
        if !self.known_peers.contains(&sender) {
            self.known_peers.push(sender);
        }
        self.scroll = self.messages.len().saturating_sub(1);
    }

    pub fn add_system_message(&mut self, text: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages.push(ChatMessage {
            sender: self.my_peer_id,
            text: format!("[SYS] {}", text),
            time,
            is_system: true,
        });
        if self.messages.len() > 500 {
            self.messages.remove(0);
        }
        self.scroll = self.messages.len().saturating_sub(1);
    }

    /// Регистрирует пира с его username
    pub fn register_peer(&mut self, username: String, peer_id: PeerId) {
        self.username_to_peer.insert(username.clone(), peer_id);
        self.peer_to_username.insert(peer_id, username.clone());
        if !self.known_peers.contains(&peer_id) {
            self.known_peers.push(peer_id);
        }
    }

    /// Ищет PeerId по username
    pub fn find_peer_by_username(&self, username: &str) -> Option<PeerId> {
        self.username_to_peer.get(username).copied()
    }

    /// Получает username по PeerId
    pub fn get_username(&self, peer_id: &PeerId) -> Option<String> {
        self.peer_to_username.get(peer_id).cloned()
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<(String, App)> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(250);

    loop {
        terminal.draw(|f| ui(f, &app))?;
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => return Ok(("/quit".to_string(), app)),
                            KeyCode::Char('e') => {
                                app.input_mode = InputMode::Editing;
                                app.status = "Editing mode (ESC=Cancel, Enter=Send)".to_string();
                            }
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Up => app.scroll = app.scroll.saturating_sub(1),
                            KeyCode::Down => app.scroll += 1,
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => {
                                let input = app.input.trim().to_string();
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready".to_string();
                                if !input.is_empty() {
                                    return Ok((input, app));
                                }
                            }
                            KeyCode::Esc => {
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready".to_string();
                            }
                            KeyCode::Char(c) => app.input.push(c),
                            KeyCode::Backspace => {
                                app.input.pop();
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.size());

    let title = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled(" 🌊 ", Style::default().fg(Color::Cyan)),
        Span::styled(
            "UNDERTOW P2P",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" — {} ", app.username),
            Style::default().fg(Color::LightGreen),
        ),
        Span::styled("| Powered by Iroh", Style::default().fg(Color::Gray)),
    ])]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(title, chunks[0]);

    let info_text = format!(
        "👤 {} ({})\n🔑 PeerID: {}",
        app.username,
        app.my_peer_id.short(),
        app.my_peer_id.to_hex()
    );
    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .title(" Node Info (Share your PeerID to connect) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(info, chunks[1]);

    let messages_text: Vec<Line> = app
        .messages
        .iter()
        .map(|msg| {
            if msg.is_system {
                return Line::from(vec![
                    Span::styled(
                        format!("[{}] ", msg.time),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        &msg.text,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]);
            }
            let is_me = msg.sender == app.my_peer_id;
            let color = if is_me { Color::Green } else { Color::Cyan };
            let prefix = if is_me {
                "You"
            } else {
                &app.get_username(&msg.sender)
                    .unwrap_or_else(|| msg.sender.short())
            };
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", msg.time),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}: ", prefix),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&msg.text, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let messages = Paragraph::new(Text::from(messages_text))
        .block(
            Block::default()
                .title(format!(" Chat [{} peers known] ", app.known_peers.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: true })
        .scroll((app.scroll as u16, 0));
    f.render_widget(messages, chunks[2]);

    let input_style = if app.input_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let input_label = if app.input_mode == InputMode::Editing {
        " Send message (ESC=Cancel) "
    } else {
        " 'e'=write, 'h'=help, 'q'=quit "
    };
    let input = Paragraph::new(app.input.clone()).style(input_style).block(
        Block::default()
            .title(input_label)
            .borders(Borders::ALL)
            .border_style(input_style),
    );
    f.render_widget(input, chunks[3]);

    let status = Paragraph::new(format!(" {} ", app.status))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[4]);

    if app.show_help {
        let help_text = "\
        🌊 UNDERTOW P2P — Help\n\n\
        Keys:\n  e=write, Enter=send, ESC=cancel, h=help, ↑↓=scroll, q=quit\n\n\
        Commands:\n\
          /msg <username> <msg>  — Send to user by name\n\
          /send <peer_id> <msg>  — Send by full PeerId (global)\n\
          /peers                 — List known peers\n\
          /id                    — Show my full PeerID\n\
          /help                  — Show this help\n";
        let area = centered_rect(60, 60, f.size());
        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(Clear, area);
        f.render_widget(help, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
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

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
use std::io;

pub struct App {
    pub peer_id: String,
    pub peer_id_short: String,
    pub username: String,
    pub local_addrs: Vec<String>,
    pub external_addr: Option<String>,
    pub nat_type: String,
    pub beacon_addr: Option<String>,
    pub lan_mode: bool,
    pub messages: Vec<(String, String, String)>, // (sender_short, sender_full, text_with_time)
    pub input: String,
    pub input_mode: InputMode,
    pub scroll: usize,
    pub connected_peers: Vec<String>,
    pub connected_peers_info: Vec<(String, String)>, // (peer_id, username)
    pub lan_peer_count: usize,
    pub status: String,
    pub show_help: bool,
    pub show_peers: bool,
    pub mode: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

impl App {
    pub fn new(
        peer_id: String,
        username: String,
        local_addrs: Vec<String>,
        external_addr: Option<String>,
        nat_type: String,
        beacon_addr: Option<String>,
    ) -> Self {
        let peer_id_short = peer_id.chars().take(8).collect();
        let lan_mode = beacon_addr.is_none();
        let mode = if lan_mode { "🏠 LAN" } else { "🗼 Beacon" };
        Self {
            peer_id,
            peer_id_short,
            username,
            local_addrs,
            external_addr,
            nat_type,
            beacon_addr,
            lan_mode,
            messages: Vec::new(),
            input: String::new(),
            input_mode: InputMode::Normal,
            scroll: 0,
            connected_peers: Vec::new(),
            connected_peers_info: Vec::new(),
            lan_peer_count: 0,
            status: "Ready / Готов".to_string(),
            show_help: false,
            show_peers: false,
            mode: mode.to_string(),
        }
    }

    pub fn add_message(&mut self, sender_full: String, text: String) {
        let sender_short: String = sender_full.chars().take(8).collect();
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages
            .push((sender_short, sender_full, format!("[{}] {}", time, text)));
        if self.messages.len() > 500 {
            self.messages.remove(0);
        }
        self.scroll = self.messages.len().saturating_sub(1);
    }

    pub fn add_system_message(&mut self, text: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages.push((
            "SYS".to_string(),
            "SYSTEM".to_string(),
            format!("[{}] {}", time, text),
        ));
        self.scroll = self.messages.len().saturating_sub(1);
    }

    pub fn add_peer(&mut self, peer_id: String, username: String) {
        let short_id = if peer_id.len() > 8 {
            format!("{}...", &peer_id[..8])
        } else {
            peer_id.clone()
        };
        let display = format!("{} (@{})", short_id, username);
        if !self.connected_peers.contains(&display) {
            self.connected_peers.push(display);
            self.connected_peers_info.push((peer_id, username));
        }
    }

    pub fn remove_peer(&mut self, peer_id: &str) {
        self.connected_peers_info.retain(|(id, _)| id != peer_id);
        self.connected_peers.retain(|p| !p.contains(peer_id));
    }

    pub fn update_peers(&mut self, peers: Vec<(String, String)>) {
        self.connected_peers.clear();
        self.connected_peers_info.clear();
        for (peer_id, username) in peers {
            self.add_peer(peer_id, username);
        }
    }

    pub fn set_lan_peer_count(&mut self, count: usize) {
        self.lan_peer_count = count;
    }
    pub fn total_peers(&self) -> usize {
        self.connected_peers.len()
    }
}

// 🚀 КЛЮЧЕВОЕ ИЗМЕНЕНИЕ: принимаем &mut App и возвращаем Option<String>
pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<Option<String>> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(250);
    loop {
        terminal.draw(|f| ui(f, app))?;
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => return Ok(Some("/quit".to_string())),
                            KeyCode::Char('e') => {
                                app.input_mode = InputMode::Editing;
                                app.status = "✏️ Editing mode / Режим ввода".to_string();
                            }
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Char('p') => app.show_peers = !app.show_peers,
                            KeyCode::Up => {
                                if app.scroll > 0 {
                                    app.scroll -= 1;
                                }
                            }
                            KeyCode::Down => {
                                app.scroll += 1;
                            }
                            KeyCode::Char('r') => {
                                app.status = "🔄 Refreshing...".to_string();
                            }
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => {
                                let input = app.input.trim().to_string();
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready / Готов".to_string();
                                if !input.is_empty() {
                                    return Ok(Some(input)); // Возвращаем ввод
                                }
                            }
                            KeyCode::Esc => {
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready / Готов".to_string();
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
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.size());

    let title_text = if app.lan_mode {
        format!("🏠 LAN MODE — {}", app.username)
    } else {
        format!("🗼 BEACON MODE — {}", app.username)
    };
    let title_color = if app.lan_mode {
        Color::Green
    } else {
        Color::Yellow
    };
    let title = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled(" 🌊 ", Style::default().fg(Color::Cyan)),
        Span::styled(
            "UNDERTOW",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" — {} ", title_text),
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("(Peers: {})", app.total_peers() + app.lan_peer_count),
            Style::default().fg(Color::Gray),
        ),
    ])]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(title, chunks[0]);

    let beacon_status = if let Some(ref beacon) = app.beacon_addr {
        format!("🟢 {}", beacon)
    } else {
        "🔴 None (LAN mode)".to_string()
    };
    let info_text = format!(
        "👤 {} ({})\n📡 {}\n🏠 Local: {}\n🌍 External: {} | 🔥 NAT: {}",
        app.username,
        app.peer_id_short,
        beacon_status,
        app.local_addrs.join(", "),
        app.external_addr.as_deref().unwrap_or("Unknown"),
        app.nat_type
    );
    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .title(" Node Info ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(info, chunks[1]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[2]);

    let messages_text: Vec<Line> = app
        .messages
        .iter()
        .map(|(short, _full, text)| {
            let color = if short == "SYS" {
                Color::Yellow
            } else if short == &app.peer_id_short || short == &app.username {
                Color::Green
            } else {
                Color::Cyan
            };
            let prefix = if short == &app.username || short == &app.peer_id_short {
                "You"
            } else if short == "SYS" {
                "SYS"
            } else {
                short
            };
            let style = if text.contains("🏠") || text.contains("LAN") {
                Style::default().fg(Color::LightGreen)
            } else if text.contains("🗼") || text.contains("beacon") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(vec![
                Span::styled(
                    format!("{} ", prefix),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(text.clone(), style),
            ])
        })
        .collect();
    let messages = Paragraph::new(Text::from(messages_text))
        .block(
            Block::default()
                .title(format!(" Chat [{}] ", app.messages.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: true })
        .scroll((app.scroll as u16, 0));
    f.render_widget(messages, main_chunks[0]);

    let mut peers_text = String::new();
    if !app.connected_peers.is_empty() {
        peers_text.push_str(&format!(
            "🌐 Network Peers ({})\n",
            app.connected_peers.len()
        ));
        for peer in &app.connected_peers {
            peers_text.push_str(&format!("  • {}\n", peer));
        }
    }
    if app.lan_peer_count > 0 {
        peers_text.push_str(&format!("\n🏠 LAN Peers ({})\n", app.lan_peer_count));
    }
    if peers_text.is_empty() {
        peers_text = "No peers connected\nНет подключённых пиров".to_string();
        if app.lan_mode {
            peers_text.push_str("\n🔍 Searching LAN...");
        }
    }
    let peers = Paragraph::new(peers_text)
        .block(
            Block::default()
                .title(format!(
                    " Peers [{}] ",
                    app.total_peers() + app.lan_peer_count
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(peers, main_chunks[1]);

    let input_style = if app.input_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let input_label = if app.input_mode == InputMode::Editing {
        " ✏️ Type message (ESC to cancel) "
    } else {
        " [e]dit  [h]elp  [p]eers  [r]efresh  [q]uit "
    };
    let input = Paragraph::new(app.input.clone()).style(input_style).block(
        Block::default()
            .title(input_label)
            .borders(Borders::ALL)
            .border_style(input_style),
    );
    f.render_widget(input, chunks[3]);

    let status_text = if app.lan_mode {
        format!(" 🏠 LAN Mode | {}", app.status)
    } else {
        format!(" 🗼 Beacon Mode | {}", app.status)
    };
    f.render_widget(
        Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        chunks[4],
    );

    if app.show_help {
        let help_text = "🌊 UNDERTOW PROTOCOL — Help\n════════════════════════════════\n\n🎮 Keys:\n  e — Start writing\n  Enter — Send\n  ESC — Cancel\n  h — Toggle help\n  p — Toggle peers\n  ↑,↓ — Scroll chat\n  q — Quit\n\n📝 Commands:\n  /msg <user> <text>\n  /send <id> <text>\n  /peers, /lan, /nat, /help";
        let area = centered_rect(60, 60, f.size());
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(help_text)
                .block(
                    Block::default()
                        .title(" Help (h to close)")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }
    if app.show_peers {
        let mut p_text = String::from("🌐 Connected Peers\n════════════════════════════════\n\n");
        if app.connected_peers_info.is_empty() {
            p_text.push_str("  No peers connected\n");
        } else {
            for (i, (peer_id, username)) in app.connected_peers_info.iter().enumerate() {
                let short_id = if peer_id.len() > 16 {
                    format!("{}...{}", &peer_id[..8], &peer_id[peer_id.len() - 8..])
                } else {
                    peer_id.clone()
                };
                p_text.push_str(&format!("  {:<2} 🏠 @{} ({})\n", i + 1, username, short_id));
            }
        }
        p_text.push_str(&format!(
            "\n  Total: {} peers",
            app.connected_peers_info.len()
        ));
        p_text.push_str("\n\n  Press 'p' to close");
        let area = centered_rect(50, 60, f.size());
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(p_text)
                .block(
                    Block::default()
                        .title(" Peers ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
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

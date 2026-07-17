//! ## Undertow Client - P2P Node with TUI
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;

use undertow_protocol::network::{
    lan::{LanBeacon, LanEvent},
    local::LocalDiscovery,
    nat::NatDetector,
};
use undertow_protocol::storage::init_profile;

mod ui;
use crate::ui::app::{run_app, App};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = init_profile(None)?;
    let my_peer_id = profile.peer_id();
    let username = profile.username.clone();

    println!("🌊 UNDERTOW PROTOCOL — Node Setup");
    println!("👤 Username: {}", username);
    println!("🔑 Peer ID: {}", my_peer_id);

    print!("📡 Node port [9001]: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut port_input = String::new();
    std::io::stdin().read_line(&mut port_input)?;
    let port: u16 = port_input.trim().parse().unwrap_or(9001);

    let local_addrs = LocalDiscovery::get_local_addrs(port);
    println!("\n🏠 Local addresses: {:?}", local_addrs);

    println!("\n🔍 Detecting NAT...");
    let nat_info = NatDetector::detect(port).await;
    let nat_type_str = format!("{:?}", nat_info.nat_type);
    let external_str = nat_info.external_addr.map(|a| a.to_string());

    // === ИСПОЛЬЗУЕМ LanBeacon ===
    // start() УЖЕ запускает фоновую отправку Announce каждые 5 секунд!
    let lan_beacon = LanBeacon::new(*my_peer_id.as_bytes(), username.clone(), port).await?;
    let (tx, mut rx) = mpsc::unbounded_channel::<LanEvent>();
    lan_beacon.start(tx.clone()).await;
    println!("🏠 LAN Beacon started (multicast on 239.255.0.1:9003)");

    // === TUI Setup ===
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let local_addr_strs: Vec<String> = local_addrs.iter().map(|a| a.to_string()).collect();

    let mut app = App::new(
        my_peer_id.to_string(),
        username.clone(),
        local_addr_strs.clone(),
        external_str.clone(),
        nat_type_str.clone(),
        None,
    );
    app.add_system_message(format!(
        "🚀 Node started on port {} | ID: {}",
        port,
        my_peer_id.to_string()
    ));
    app.add_system_message("🔍 Searching for peers in local network...".to_string());

    // === Main Loop ===
    let mut last_status_update = std::time::Instant::now();
    let status_update_interval = std::time::Duration::from_secs(10);

    loop {
        // 1. Обрабатываем ВСЕ сетевые события (пиры находятся АВТОМАТИЧЕСКИ каждые 5 сек)
        while let Ok(event) = rx.try_recv() {
            match event {
                LanEvent::PeerJoined {
                    peer_id, username, ..
                } => {
                    let pid_hex = hex::encode(peer_id);
                    app.add_peer(pid_hex.clone(), username.clone());
                    app.add_system_message(format!(
                        "🏠 LAN peer joined: {} ({})",
                        username,
                        &pid_hex[..8]
                    ));
                }
                LanEvent::PeerLeft { peer_id, username } => {
                    let pid_hex = hex::encode(peer_id);
                    app.remove_peer(&pid_hex);
                    app.add_system_message(format!(
                        "🏠 LAN peer left: {} ({})",
                        username,
                        &pid_hex[..8]
                    ));
                }
                LanEvent::ChatMessage {
                    sender_id,
                    sender_name: _,
                    content,
                } => {
                    let pid_hex = hex::encode(sender_id);
                    app.add_message(pid_hex, content);
                }
            }
        }
        app.set_lan_peer_count(app.total_peers());

        // 2. Периодическое обновление статуса (каждые 10 секунд)
        if last_status_update.elapsed() >= status_update_interval {
            let peer_count = app.total_peers();

            if peer_count > 0 {
                app.status = format!("🌐 {} peers connected", peer_count);
            } else {
                app.status = "🔍 Scanning for peers...".to_string();
            }
            last_status_update = std::time::Instant::now();
        }

        // 3. Отрисовываем UI (состояние сохраняется благодаря &mut app)
        let input_opt = run_app(&mut terminal, &mut app)?;

        // 4. Обрабатываем ввод пользователя
        if let Some(input) = input_opt {
            if input == "/quit" {
                lan_beacon.stop().await;
                break;
            }

            if input == "/help" || input == "help" {
                app.show_help = true;
            } else if input == "/peers" || input == "p" {
                app.show_peers = !app.show_peers;
            } else if input == "/lan" {
                // РУЧНОЙ ЗАПРОС - показываем список из уже найденных пиров
                let peers_snapshot: Vec<(String, String)> = app
                    .connected_peers_info
                    .iter()
                    .map(|(id, name)| (id.clone(), name.clone()))
                    .collect();

                if peers_snapshot.is_empty() {
                    app.add_system_message(
                        "🔍 No LAN peers found yet. Waiting for Announce...".to_string(),
                    );
                } else {
                    app.add_system_message(format!(
                        "🏠 Active LAN peers: {}",
                        peers_snapshot.len()
                    ));
                    for (id, name) in &peers_snapshot {
                        app.add_system_message(format!("  • {} ({})", name, &id[..8]));
                    }
                }
            } else if input == "/nat" {
                let nat = NatDetector::detect(port).await;
                app.add_system_message(format!(
                    "NAT: {:?} | External: {:?}",
                    nat.nat_type, nat.external_addr
                ));
            } else if !input.starts_with('/') {
                // ОТПРАВКА СООБЩЕНИЯ (Broadcast)
                if app.total_peers() == 0 {
                    app.add_system_message("⚠️ No peers connected. Message not sent.".to_string());
                } else {
                    match lan_beacon.broadcast_chat_message(input.clone()).await {
                        Ok(_) => {
                            app.add_message(my_peer_id.to_string(), input.clone());
                        }
                        Err(e) => {
                            app.add_system_message(format!("❌ Broadcast failed: {}", e));
                        }
                    }
                }
            } else {
                app.add_system_message(format!("❌ Unknown command: {}. Type /help", input));
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    println!("\n👋 Goodbye!");
    Ok(())
}

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

    // === ИСПОЛЬЗУЕМ LanBeacon ВМЕСТО LanDiscovery ===
    let lan_beacon = LanBeacon::new(*my_peer_id.as_bytes(), username.clone(), port).await?;
    let (tx, mut rx) = mpsc::unbounded_channel::<LanEvent>();
    lan_beacon.start(tx.clone()).await;
    println!("🏠 LAN Beacon started (multicast on 239.255.0.1:9003)");

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

    loop {
        // 1. Обрабатываем ВСЕ сетевые события (пиры находятся АВТОМАТИЧЕСКИ)
        while let Ok(event) = rx.try_recv() {
            match event {
                LanEvent::PeerJoined {
                    peer_id,
                    username,
                    addr: _,
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
        app.set_lan_peer_count(app.connected_peers_info.len());

        // 2. Отрисовываем UI. Передаем &mut app, чтобы состояние НЕ терялось!
        let input_opt = run_app(&mut terminal, &mut app)?;

        // 3. Обрабатываем ввод пользователя
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
                let peers_snapshot: Vec<(String, String)> = app
                    .connected_peers_info
                    .iter()
                    .map(|(id, name)| (id.clone(), name.clone()))
                    .collect();
                app.add_system_message(format!("🏠 Active LAN peers: {}", peers_snapshot.len()));
                for (id, name) in &peers_snapshot {
                    app.add_system_message(format!("  • {} ({})", name, &id[..8]));
                }
            } else if input.starts_with("/chat ") || input.starts_with("/dm ") {
                let target = input[6..].trim().to_lowercase();
                let found = app
                    .connected_peers_info
                    .iter()
                    .find(|(id, name)| {
                        id.to_lowercase().starts_with(&target) || name.to_lowercase() == target
                    })
                    .map(|(id, _)| id.clone());
                if let Some(pid) = found {
                    let uname = app
                        .connected_peers_info
                        .iter()
                        .find(|(id, _)| id == &pid)
                        .unwrap()
                        .1
                        .clone();
                    app.select_chat(Some(pid.clone()));
                    app.add_system_message(format!("💬 Switched to DM with @{}", uname));
                } else {
                    app.add_system_message(format!("❌ Peer '{}' not found", target));
                }
            } else if input == "/broadcast" || input == "/bcast" {
                app.switch_to_broadcast();
            } else if input == "/nat" {
                let nat = NatDetector::detect(port).await;
                app.add_system_message(format!(
                    "NAT: {:?} | External: {:?}",
                    nat.nat_type, nat.external_addr
                ));
            } else if !input.starts_with('/') {
                // ОТПРАВКА СООБЩЕНИЯ БЕЗ КОМАНДЫ
                if let Some(ref target_id_hex) = app.selected_chat {
                    if let Ok(target_bytes) = hex::decode(target_id_hex) {
                        let mut target_array = [0u8; 32];
                        target_array.copy_from_slice(&target_bytes);
                        match lan_beacon
                            .send_chat_message(target_array, input.clone())
                            .await
                        {
                            Ok(_) => {
                                app.add_message(
                                    my_peer_id.to_string(),
                                    format!("[→ DM] {}", input),
                                );
                            }
                            Err(e) => {
                                app.add_system_message(format!("❌ Send failed: {}", e));
                            }
                        }
                    }
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

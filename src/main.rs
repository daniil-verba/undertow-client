//! ## Undertow Client - P2P Node with TUI (Iroh Edition)

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;

use undertow_protocol::{
    network::IrohTransport,
    protocol::{
        packet::{Packet, PacketType},
        peer_id::PeerId,
    },
    storage::init_profile,
};

mod ui;
use crate::ui::app::{run_app, App, UiEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = init_profile(None)?;
    let username = profile.username.clone();

    println!("🌊 UNDERTOW PROTOCOL — Iroh P2P Node");
    println!("═══════════════════════════════════════════════════");
    println!("👤 Username: {}", username);

    // Генерируем ключ. В продакшене его нужно сохранять в profile!
    let secret_key = iroh::SecretKey::generate();
    let transport = Arc::new(IrohTransport::new(Some(secret_key)).await?);
    let my_peer_id = transport.peer_id();

    println!("🔑 My Peer ID: {}", my_peer_id);
    println!("✅ Transport ready with automatic discovery (mDNS + relay)");
    println!("═══════════════════════════════════════════════════\n");

    let mut incoming_rx = transport.start_listening();
    let (ui_tx, mut ui_rx) = mpsc::channel::<UiEvent>(100);

    let transport_clone = transport.clone();
    let ui_tx_clone = ui_tx.clone();
    let my_username = username.clone();

    // Фоновый обработчик входящих пакетов
    tokio::spawn(async move {
        while let Some((sender_id, packet)) = incoming_rx.recv().await {
            match packet.packet_type {
                PacketType::Hello => {
                    if let Some(peer_username) = packet.hello_username() {
                        let _ = ui_tx_clone
                            .send(UiEvent::PeerDiscovered(sender_id, peer_username))
                            .await;
                    }
                    // Отвечаем своим username
                    let _ = transport_clone
                        .send_packet(sender_id, &Packet::hello(&my_username))
                        .await;
                }
                PacketType::Ping => {
                    let _ = transport_clone
                        .send_packet(sender_id, &Packet::pong())
                        .await;
                    let _ = ui_tx_clone
                        .send(UiEvent::SystemMessage(format!(
                            "📡 Ping from {}",
                            sender_id.short()
                        )))
                        .await;
                }
                PacketType::Pong => {
                    let _ = ui_tx_clone
                        .send(UiEvent::SystemMessage(format!(
                            "✅ Pong from {}",
                            sender_id.short()
                        )))
                        .await;
                }
                PacketType::Message => {
                    if let Some(content) = packet.message_content() {
                        let text = String::from_utf8_lossy(content).to_string();
                        let _ = ui_tx_clone
                            .send(UiEvent::MessageReceived(sender_id, text))
                            .await;
                    }
                }
                _ => {}
            }
        }
    });

    // Инициализация TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(my_peer_id, username.clone());
    app.add_system_message(format!("Node started. ID: {}", my_peer_id.short()));
    app.add_system_message("🌐 Automatic discovery active (mDNS + iroh relay)".to_string());
    app.add_system_message("Peers will appear automatically when they send Hello".to_string());
    app.add_system_message("Type /help for commands.".to_string());

    // Главный цикл
    loop {
        // Обработка событий из сети
        while let Ok(event) = ui_rx.try_recv() {
            match event {
                UiEvent::MessageReceived(sender, text) => {
                    app.add_message(sender, text);
                }
                UiEvent::SystemMessage(text) => {
                    app.add_system_message(text);
                }
                UiEvent::Error(text) => {
                    app.add_system_message(format!("❌ {}", text));
                }
                UiEvent::PeerDiscovered(peer_id, peer_username) => {
                    app.register_peer(peer_username.clone(), peer_id);
                    app.add_system_message(format!(
                        "🌐 Peer discovered: @{} ({})",
                        peer_username,
                        peer_id.short()
                    ));
                }
            }
        }

        let (input, new_app) = run_app(&mut terminal, app)?;
        app = new_app;

        if input == "/quit" {
            break;
        }

        if input == "/help" {
            app.show_help = true;
        } else if input.starts_with("/msg ") {
            let parts: Vec<&str> = input[5..].splitn(2, ' ').collect();
            if parts.len() == 2 {
                let target_username = parts[0];
                let message = parts[1];

                if let Some(recipient) = app.find_peer_by_username(target_username) {
                    let packet = Packet::message(&my_peer_id, &recipient, message.as_bytes());
                    match transport.send_packet(recipient, &packet).await {
                        Ok(_) => {
                            app.add_system_message(format!(
                                "📤 Sent to @{}: {}",
                                target_username, message
                            ));
                            app.add_message(
                                my_peer_id,
                                format!("[→ @{}] {}", target_username, message),
                            );
                        }
                        Err(e) => app.add_system_message(format!("Send failed: {}", e)),
                    }
                } else {
                    app.add_system_message(format!("❌ User @{} not found", target_username));
                    app.add_system_message(
                        "💡 Wait for peer to connect and send Hello".to_string(),
                    );
                }
            } else {
                app.add_system_message("Usage: /msg <username> <message>".to_string());
            }
        } else if input == "/peers" {
            let count = app.username_to_peer.len();
            app.add_system_message(format!("Known peers: {}", count));
            let peers_copy = app.username_to_peer.clone();
            for (uname, p_id) in peers_copy {
                app.add_system_message(format!("  🟢 @{} ({})", uname, p_id.short()));
            }
        } else if input == "/id" {
            app.add_system_message(format!("🔑 My full Peer ID: {}", my_peer_id.to_hex()));
        } else if !input.is_empty() && !input.starts_with('/') {
            app.add_system_message(format!("Unknown command: {}. Type /help", input));
        }
    }

    transport.shutdown().await;
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

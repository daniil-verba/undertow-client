//! ## Undertow Client - P2P Node with TUI

use std::io;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use undertow_protocol::{
    beacon::beacon_client::BeaconClient,
    network::lan_beacon::{LanBeacon, LanEvent},
    network::{local::LocalDiscovery, nat::NatDetector},
    protocol::{
        packet::{Packet, PacketType},
        peer_id::PeerId,
    },
    storage::init_profile,
};

mod ui;
use crate::ui::app::{run_app, App};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === PHASE 0: Load or create profile ===============================
    let mut profile = init_profile(None)?;
    let my_peer_id = profile.peer_id();
    let username = profile.username.clone();

    println!("🌊 UNDERTOW PROTOCOL — Node Setup");
    println!("═══════════════════════════════════════════════════");
    println!("👤 Username: {}", username);
    println!("🔑 Peer ID: {}", my_peer_id);

    // === PHASE 1: CLI Setup ===========================================
    print!("📡 Node port [9001]: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut port_input = String::new();
    std::io::stdin().read_line(&mut port_input)?;
    let port: u16 = port_input.trim().parse().unwrap_or(9001);

    print!("🗼 Beacon address [ip:port or 'none']: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut beacon_input = String::new();
    std::io::stdin().read_line(&mut beacon_input)?;
    let beacon_addr = beacon_input.trim().to_string();
    let beacon_addr_opt = if beacon_addr == "none" || beacon_addr.is_empty() {
        None
    } else {
        Some(beacon_addr)
    };

    let local_addrs = LocalDiscovery::get_local_addrs(port);
    println!("\n🏠 Local addresses: {:?}", local_addrs);

    println!("\n🔍 Detecting NAT...");
    let nat_info = NatDetector::detect(port).await;
    let nat_type_str = format!("{:?}", nat_info.nat_type);
    let external_str = nat_info.external_addr.map(|a| a.to_string());

    // Start TCP server
    let addr = format!("0.0.0.0:{}", port);
    let _server = tokio::spawn(run_server(addr.clone(), my_peer_id));

    // Connect to beacon with username
    let mut beacon_client: Option<BeaconClient> = None;
    if let Some(ref beacon) = beacon_addr_opt {
        match BeaconClient::connect(beacon, my_peer_id, &username).await {
            Ok(client) => {
                println!("✅ Connected to beacon: {}", beacon);
                beacon_client = Some(client);
            }
            Err(e) => {
                println!("⚠️ Beacon connection failed: {}", e);
            }
        }
    }

    // === PHASE 2: TUI =================================================
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
        beacon_addr_opt.clone(),
    );

    app.add_system_message(format!(
        "Node started on port {} | ID: {}",
        port,
        my_peer_id.short()
    ));

    if let Some(ref beacon) = beacon_addr_opt {
        if beacon_client.is_some() {
            app.add_system_message(format!("Connected to beacon: {}", beacon));
        } else {
            app.add_system_message(format!("Beacon connection failed: {}", beacon));
        }
    }

    // === PHASE 3: LAN Beacon Setup =====================================
    let peer_id_bytes = *my_peer_id.as_bytes();
    let lan_beacon = Arc::new(LanBeacon::new(peer_id_bytes, username.clone(), port).await?);

    let (lan_tx, mut lan_rx) = mpsc::unbounded_channel::<LanEvent>();

    lan_beacon.start(lan_tx.clone()).await;

    app.lan_beacon = Some(lan_beacon.clone());
    app.add_system_message("🌐 LAN beacon started on port 9003".to_string());

    // Channel for async messages
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Spawn beacon receiver
    if let Some(mut client) = beacon_client {
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                match client.receive_packet().await {
                    Ok(packet) => {
                        if packet.packet_type == PacketType::Message {
                            if let Some(sender) = packet.message_sender() {
                                if let Some(content) = packet.message_content() {
                                    let text = String::from_utf8_lossy(content).to_string();
                                    let _ = tx_clone.send(format!("MSG:{}:{}", sender, text));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx_clone.send(format!("ERR:Beacon read error: {}", e));
                        break;
                    }
                }
            }
        });
    }

    // Reconnect beacon for sending
    let mut beacon_client: Option<BeaconClient> = None;
    if let Some(ref beacon) = beacon_addr_opt {
        if let Ok(client) = BeaconClient::connect(beacon, my_peer_id, &username).await {
            beacon_client = Some(client);
        }
    }

    // Main loop
    loop {
        // === PROCESS MESSAGES FROM BEACON ===
        while let Ok(msg) = rx.try_recv() {
            if msg.starts_with("MSG:") {
                let parts: Vec<&str> = msg[4..].splitn(2, ':').collect();
                if parts.len() == 2 {
                    let content = parts[1];
                    if content.starts_with("PEER_LIST:") {
                        let list_data = &content[10..];
                        if !list_data.is_empty() {
                            let mut peers = Vec::new();
                            for entry in list_data.split(';') {
                                let parts: Vec<&str> = entry.splitn(2, ':').collect();
                                if parts.len() == 2 {
                                    peers.push((parts[0].to_string(), parts[1].to_string()));
                                }
                            }
                            app.update_peers(peers);
                            app.add_system_message(format!(
                                "📋 Updated peer list: {} peers",
                                app.connected_peers.len()
                            ));
                        }
                    } else if content.starts_with("PEER_JOINED:") {
                        let parts: Vec<&str> = content[12..].splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let peer_id = parts[0].to_string();
                            let username = parts[1].to_string();
                            app.add_peer(peer_id.clone(), username.clone());
                            app.add_system_message(format!(
                                "👤 {} ({}) joined the network",
                                username,
                                &peer_id[..8]
                            ));
                        }
                    } else if content.starts_with("PEER_LEFT:") {
                        let parts: Vec<&str> = content[10..].splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let peer_id = parts[0].to_string();
                            let username = parts[1].to_string();
                            app.remove_peer(&peer_id);
                            app.add_system_message(format!(
                                "👋 {} ({}) left the network",
                                username,
                                &peer_id[..8]
                            ));
                        }
                    } else {
                        app.add_message(parts[0].to_string(), content.to_string());
                    }
                }
            } else if msg.starts_with("ERR:") {
                app.add_system_message(msg[4..].to_string());
            }
        }

        // === PROCESS LAN EVENTS ===
        while let Ok(event) = lan_rx.try_recv() {
            match event {
                LanEvent::PeerJoined {
                    peer_id,
                    username,
                    addr,
                } => {
                    let peer_id_hex = hex::encode(&peer_id);
                    app.add_lan_peer(peer_id_hex.clone(), username.clone());
                    app.add_system_message(format!(
                        "🌐 LAN: {} (@{}) joined from {}",
                        username,
                        &peer_id_hex[..8],
                        addr
                    ));
                }
                LanEvent::PeerLeft { peer_id, username } => {
                    let peer_id_hex = hex::encode(&peer_id);
                    app.remove_lan_peer(&peer_id_hex);
                    app.add_system_message(format!(
                        "🌐 LAN: {} (@{}) left",
                        username,
                        &peer_id_hex[..8]
                    ));
                }
                LanEvent::ChatMessage {
                    sender_id,
                    sender_name,
                    content,
                } => {
                    let sender_hex = hex::encode(&sender_id);
                    app.add_message(sender_hex, format!("[LAN] {}", content));
                    app.add_system_message(format!("💬 LAN from {}: {}", sender_name, content));
                }
            }
        }

        // === RUN TUI ===
        let (input, new_app) = run_app(&mut terminal, app)?;
        app = new_app;

        if input == "/quit" {
            break;
        }

        // === HANDLE COMMANDS ===
        if input == "/help" || input == "help" {
            app.show_help = true;
            continue;
        } else if input == "/peers" {
            if let Some(ref mut _client) = beacon_client {
                app.add_system_message("Peer list requested (not implemented yet)".to_string());
            } else {
                app.add_system_message("Not connected to beacon".to_string());
            }
            continue;
        } else if input == "/name" {
            app.start_change_name();
            continue;
        } else if input.starts_with("/name ") {
            let new_name = input[6..].trim().to_string();
            if !new_name.is_empty() && new_name.len() <= 32 {
                match profile.update_username(new_name.clone()) {
                    Ok(()) => {
                        app.username = new_name.clone();
                        app.add_system_message(format!("✅ Username changed to: {}", new_name));
                        lan_beacon.update_username(new_name).await;
                    }
                    Err(e) => {
                        app.add_system_message(format!("❌ Failed to change name: {}", e));
                    }
                }
            } else {
                app.add_system_message("❌ Invalid username (must be 1-32 characters)".to_string());
            }
            continue;
        } else if input == "/lan" {
            let peers = lan_beacon.get_active_peers().await;
            if peers.is_empty() {
                app.add_system_message("🌐 No LAN peers found".to_string());
            } else {
                app.add_system_message(format!("🌐 LAN peers ({}):", peers.len()));
                for peer in peers {
                    app.add_system_message(format!(
                        "  🟢 {} (@{})",
                        peer.username,
                        hex::encode(&peer.peer_id)[..8].to_string()
                    ));
                }
            }
            continue;
        } else if input.starts_with("/msg-lan ") {
            let parts: Vec<&str> = input[9..].splitn(2, ' ').collect();
            if parts.len() == 2 {
                let target_name = parts[0];
                let content = parts[1];

                let peers = lan_beacon.get_active_peers().await;
                if let Some(peer) = peers.iter().find(|p| p.username == target_name) {
                    match lan_beacon
                        .send_chat_message(peer.peer_id, content.to_string())
                        .await
                    {
                        Ok(_) => {
                            app.add_system_message(format!(
                                "📤 LAN message sent to {}: {}",
                                target_name, content
                            ));
                            app.add_message(
                                app.peer_id.clone(),
                                format!("[→ LAN @{}] {}", target_name, content),
                            );
                        }
                        Err(e) => {
                            app.add_system_message(format!("❌ Failed to send LAN message: {}", e));
                        }
                    }
                } else {
                    app.add_system_message(format!("❌ Peer not found in LAN: {}", target_name));
                }
            } else {
                app.add_system_message("Usage: /msg-lan <username> <message>".to_string());
            }
            continue;
        } else if input.starts_with("/broadcast") {
            let content = if input.len() > 10 {
                input[10..].trim()
            } else {
                app.add_system_message(
                    "💡 Type /broadcast <message> to send to all LAN peers".to_string(),
                );
                continue;
            };

            if !content.is_empty() {
                match lan_beacon.broadcast_chat_message(content.to_string()).await {
                    Ok(_) => {
                        app.add_system_message("📢 Broadcast sent to all LAN peers".to_string());
                        app.add_message(app.peer_id.clone(), format!("[📢 BROADCAST] {}", content));
                    }
                    Err(e) => {
                        app.add_system_message(format!("❌ Failed to broadcast: {}", e));
                    }
                }
            }
            continue;
        } else if input.starts_with("/msg ") {
            let parts: Vec<&str> = input[5..].splitn(2, ' ').collect();
            if parts.len() == 2 {
                let target_username = parts[0];
                let message = parts[1];

                if let Some(ref mut client) = beacon_client {
                    match client.send_to_username(target_username, message).await {
                        Ok(_) => {
                            app.add_system_message(format!(
                                "Sent to @{}: {}",
                                target_username, message
                            ));
                            app.add_message(
                                my_peer_id.to_string(),
                                format!("[→ @{}] {}", target_username, message),
                            );
                        }
                        Err(e) => {
                            app.add_system_message(format!("Send failed: {}", e));
                        }
                    }
                } else {
                    app.add_system_message("Not connected to beacon".to_string());
                }
            } else {
                app.add_system_message("Usage: /msg <username> <message>".to_string());
            }
            continue;
        } else if input.starts_with("/send ") {
            let parts: Vec<&str> = input[6..].splitn(2, ' ').collect();
            if parts.len() == 2 {
                match PeerId::from_hex(parts[0]) {
                    Ok(recipient) => {
                        let packet = Packet::message(&my_peer_id, &recipient, parts[1].as_bytes());
                        if let Some(ref mut client) = beacon_client {
                            match client.send_packet(&packet).await {
                                Ok(_) => {
                                    app.add_system_message(format!(
                                        "Sent to {}: {}",
                                        recipient.short(),
                                        parts[1]
                                    ));
                                    app.add_message(
                                        my_peer_id.to_string(),
                                        format!("[→ {}] {}", recipient.short(), parts[1]),
                                    );
                                }
                                Err(e) => {
                                    app.add_system_message(format!("Send failed: {}", e));
                                }
                            }
                        } else {
                            app.add_system_message("Not connected to beacon".to_string());
                        }
                    }
                    Err(e) => {
                        app.add_system_message(format!("Invalid PeerId: {}", e));
                    }
                }
            } else {
                app.add_system_message("Usage: /send <peer_id> <message>".to_string());
            }
            continue;
        } else if input == "/nat" {
            let nat = NatDetector::detect(port).await;
            app.add_system_message(format!(
                "NAT: {:?} | External: {:?}",
                nat.nat_type, nat.external_addr
            ));
            continue;
        } else if input.starts_with("/ping ") {
            let target = input[6..].trim();
            match send_ping(target).await {
                Ok(_) => app.add_system_message(format!("Ping OK: {}", target)),
                Err(e) => app.add_system_message(format!("Ping failed: {}", e)),
            }
            continue;
        } else if !input.is_empty() && !input.starts_with('/') {
            app.add_system_message(format!("Unknown command: {}", input));
        }
    }

    // === CLEANUP ===
    lan_beacon.stop().await;
    app.add_system_message("🌐 LAN beacon stopped".to_string());

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

/// TCP server for incoming connections.
async fn run_server(addr: String, _my_id: PeerId) {
    use tokio::net::TcpListener;

    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("❌ Bind failed: {}", e);
            return;
        }
    };

    loop {
        let (mut stream, peer_addr) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("❌ Accept error: {}", e);
                continue;
            }
        };

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            if let Ok(n) = stream.read(&mut buf).await {
                if n >= 4 {
                    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                    if let Ok(packet) = Packet::deserialize(&buf[4..4 + len]) {
                        match packet.packet_type {
                            PacketType::Ping => {
                                let pong = Packet::pong();
                                let bytes = pong.serialize();
                                let _ = stream.write_all(&(bytes.len() as u32).to_be_bytes()).await;
                                let _ = stream.write_all(&bytes).await;
                            }
                            PacketType::Message => {
                                if let Some(sender) = packet.message_sender() {
                                    if let Some(content) = packet.message_content() {
                                        let text = String::from_utf8_lossy(content);
                                        println!(
                                            "💬 [{}] Message from {}: {}",
                                            peer_addr,
                                            sender.short(),
                                            text
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    }
}

/// Sends a ping packet to target address.
async fn send_ping(addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(addr).await?;

    let ping = Packet::ping();
    let bytes = ping.serialize();
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&bytes).await?;

    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).await?;
    if n < 4 {
        return Err("No response".into());
    }

    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let resp = Packet::deserialize(&buf[4..4 + len])?;

    if resp.packet_type == PacketType::Pong {
        Ok(())
    } else {
        Err("Expected Pong".into())
    }
}

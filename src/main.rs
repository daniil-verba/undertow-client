//! ## Undertow Client - P2P Node with TUI
//!
//! Main entry point for the P2P node with beautiful TUI.
//!
//! ## Usage / Использование:
//! ```bash
//! cargo run --bin undertow
//! ```

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

// Исправленный импорт: beacon находится на верхнем уровне
use undertow_protocol::beacon::beacon_client::BeaconClient;
use undertow_protocol::network::{
    lan_discovery::{LanDiscovery, LanMessageType},
    local::LocalDiscovery,
    nat::NatDetector,
};
use undertow_protocol::protocol::{
    packet::{Packet, PacketType},
    peer_id::PeerId,
};
use undertow_protocol::storage::init_profile;

mod ui;
use crate::ui::app::{run_app, App};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === PHASE 0: Load or create profile ===============================
    let profile = init_profile(None)?;
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

    print!("🗼 Beacon address [ip:port, 'none' for LAN only]: ");
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

    // Start TCP server for incoming connections
    let addr = format!("0.0.0.0:{}", port);
    let _server = tokio::spawn(run_server(addr.clone(), my_peer_id));

    // === PHASE 2: Setup LAN Discovery ==================================
    let lan_discovery = LanDiscovery::new(my_peer_id, username.clone(), port).await?;

    // Channel for async messages (UI, LAN, Beacon)
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Clone lan_discovery BEFORE moving into the closure
    let lan_discovery_clone = lan_discovery.clone();
    let peer_id_clone = my_peer_id.clone();
    let tx_lan = tx.clone();

    // Start LAN discovery listener
    tokio::spawn(async move {
        let _ = lan_discovery_clone
            .listen(move |msg, addr| {
                // Ignore our own messages
                if msg.peer_id == peer_id_clone.to_string() {
                    return;
                }

                match msg.msg_type {
                    LanMessageType::Announce => {
                        // Found a peer in LAN
                        let peer_addr = SocketAddr::new(addr.ip(), msg.port);
                        let _ = tx_lan.send(format!(
                            "LAN_PEER_FOUND:{}:{}:{}",
                            msg.peer_id, msg.username, peer_addr
                        ));
                    }
                    LanMessageType::Leave => {
                        let _ =
                            tx_lan.send(format!("LAN_PEER_LEFT:{}:{}", msg.peer_id, msg.username));
                    }
                }
            })
            .await;
    });

    // Start periodic LAN announcements (clone again)
    let lan_announce = lan_discovery.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let _ = lan_announce.announce().await;
        }
    });

    // Send initial announce (use original)
    let _ = lan_discovery.announce().await;
    println!("🏠 LAN Discovery started (announcing every 10s)");

    // === PHASE 3: Connect to Beacon (if specified) =====================
    let mut beacon_client: Option<BeaconClient> = None;
    if let Some(ref beacon) = beacon_addr_opt {
        match BeaconClient::connect(beacon, my_peer_id).await {
            Ok(client) => {
                println!("✅ Connected to beacon: {}", beacon);
                beacon_client = Some(client);
            }
            Err(e) => {
                println!("⚠️ Beacon connection failed: {}", e);
            }
        }
    }

    // === PHASE 4: TUI Setup ============================================
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
        "🚀 Node started on port {} | ID: {}",
        port,
        my_peer_id.short()
    ));

    if beacon_addr_opt.is_some() {
        if beacon_client.is_some() {
            app.add_system_message("🗼 Connected to beacon".to_string());
        } else {
            app.add_system_message("⚠️ Beacon connection failed".to_string());
        }
    } else {
        app.add_system_message("🏠 LAN mode: beacon not used".to_string());
        app.add_system_message("🔍 Searching for peers in local network...".to_string());
    }

    // === PHASE 5: Beacon message receiver ==============================
    if let Some(mut client) = beacon_client {
        let tx_beacon = tx.clone();
        tokio::spawn(async move {
            loop {
                match client.receive_packet().await {
                    Ok(packet) => {
                        if packet.packet_type == PacketType::Message {
                            if let Some(sender) = packet.message_sender() {
                                if let Some(content) = packet.message_content() {
                                    let text = String::from_utf8_lossy(content).to_string();
                                    let _ = tx_beacon.send(format!("MSG:{}:{}", sender, text));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx_beacon.send(format!("ERR:Beacon read error: {}", e));
                        break;
                    }
                }
            }
        });
    }

    // Reconnect beacon for sending (separate connection)
    let mut beacon_sender: Option<BeaconClient> = None;
    if let Some(ref beacon) = beacon_addr_opt {
        if let Ok(client) = BeaconClient::connect(beacon, my_peer_id).await {
            beacon_sender = Some(client);
        }
    }

    // === PHASE 6: Main TUI Loop =========================================
    let mut lan_peers: HashMap<String, String> = HashMap::new();

    loop {
        // Process pending messages
        while let Ok(msg) = rx.try_recv() {
            if msg.starts_with("MSG:") {
                let parts: Vec<&str> = msg[4..].splitn(2, ':').collect();
                if parts.len() == 2 {
                    let sender = parts[0].to_string();
                    let content = parts[1].to_string();

                    // Check for system messages from Beacon
                    if content.starts_with("PEER_LIST:") {
                        let list_data = &content[10..];
                        if !list_data.is_empty() {
                            let mut peers = Vec::new();
                            for entry in list_data.split(';') {
                                let peer_parts: Vec<&str> = entry.splitn(2, ':').collect();
                                if peer_parts.len() == 2 {
                                    let peer_id = peer_parts[0].to_string();
                                    let username = peer_parts[1].to_string();
                                    if peer_id != my_peer_id.to_string() {
                                        peers.push((peer_id, username));
                                    }
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
                            if peer_id != my_peer_id.to_string() {
                                app.add_peer(peer_id.clone(), username.clone());
                                app.add_system_message(format!(
                                    "👤 {} joined the network",
                                    username
                                ));
                            }
                        }
                    } else if content.starts_with("PEER_LEFT:") {
                        let parts: Vec<&str> = content[10..].splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let peer_id = parts[0].to_string();
                            let username = parts[1].to_string();
                            app.remove_peer(&peer_id);
                            app.add_system_message(format!("👋 {} left the network", username));
                        }
                    } else {
                        // Regular message
                        app.add_message(sender, content);
                    }
                }
            } else if msg.starts_with("LAN_PEER_FOUND:") {
                let parts: Vec<&str> = msg[15..].splitn(3, ':').collect();
                if parts.len() == 3 {
                    let peer_id = parts[0].to_string();
                    let username = parts[1].to_string();
                    let addr = parts[2].to_string();

                    // Store LAN peer
                    lan_peers.insert(peer_id.clone(), addr.clone());
                    app.set_lan_peer_count(lan_peers.len());

                    // Add to UI if not already present
                    if !app
                        .connected_peers_info
                        .iter()
                        .any(|(id, _)| id == &peer_id)
                    {
                        app.add_peer(peer_id.clone(), username.clone());
                        app.add_system_message(format!(
                            "🏠 LAN peer found: {} ({}) at {}",
                            username,
                            &peer_id[..8],
                            addr
                        ));
                    }
                }
            } else if msg.starts_with("LAN_PEER_LEFT:") {
                let parts: Vec<&str> = msg[14..].splitn(2, ':').collect();
                if parts.len() == 2 {
                    let peer_id = parts[0].to_string();
                    let username = parts[1].to_string();
                    app.remove_peer(&peer_id);
                    lan_peers.remove(&peer_id);
                    app.set_lan_peer_count(lan_peers.len());
                    app.add_system_message(format!(
                        "🏠 LAN peer left: {} ({})",
                        username,
                        &peer_id[..8]
                    ));
                }
            } else if msg.starts_with("ERR:") {
                app.add_system_message(msg[4..].to_string());
            }
        }

        // Run UI and get input
        let input = run_app(&mut terminal, app)?;

        // Recreate app with preserved state
        app = App::new(
            my_peer_id.to_string(),
            username.clone(),
            local_addr_strs.clone(),
            external_str.clone(),
            nat_type_str.clone(),
            beacon_addr_opt.clone(),
        );
        // Restore LAN peer count
        app.set_lan_peer_count(lan_peers.len());

        if input == "/quit" {
            // Send leave notification before quitting
            let _ = lan_discovery.leave().await;
            break;
        }

        // === PHASE 7: Command Handling =================================
        if input == "/help" || input == "help" {
            app.show_help = true;
            continue;
        } else if input == "/peers" {
            if beacon_sender.is_some() {
                // Request peer list from beacon
                let packet = Packet::message(&my_peer_id, &my_peer_id, b"GET_PEERS".as_ref());
                if let Some(ref mut client) = beacon_sender {
                    match client.send_packet(&packet).await {
                        Ok(_) => {
                            app.add_system_message(
                                "📋 Requesting peer list from beacon...".to_string(),
                            );
                        }
                        Err(e) => {
                            app.add_system_message(format!("❌ Failed to request peers: {}", e));
                        }
                    }
                }
            } else {
                // Show LAN peers
                if lan_peers.is_empty() {
                    app.add_system_message("🏠 No LAN peers found".to_string());
                } else {
                    app.add_system_message(format!("🏠 LAN peers: {}", lan_peers.len()));
                    for (_id, addr) in &lan_peers {
                        // _id вместо id
                        app.add_system_message(format!("  • {}", addr));
                    }
                }
            }
        } else if input.starts_with("/msg ") {
            let parts: Vec<&str> = input[5..].splitn(2, ' ').collect();
            if parts.len() == 2 {
                let target_username = parts[0];
                let message = parts[1];

                // Try to find peer by username
                let mut target_peer_id = None;
                let mut target_addr = None;

                // Check LAN peers
                for (id, addr) in &lan_peers {
                    // Check if this peer matches the username
                    if let Some(peer_info) = app
                        .connected_peers_info
                        .iter()
                        .find(|(_, name)| name == target_username)
                    {
                        target_peer_id = Some(peer_info.0.clone());
                        target_addr = Some(addr.clone());
                        break;
                    }
                }

                // Also check network peers from beacon
                if target_peer_id.is_none() {
                    if let Some(peer_info) = app
                        .connected_peers_info
                        .iter()
                        .find(|(_, name)| name == target_username)
                    {
                        target_peer_id = Some(peer_info.0.clone());
                    }
                }

                if let Some(peer_id_str) = target_peer_id {
                    // Try to parse PeerId
                    if let Ok(peer_id) = PeerId::from_hex(&peer_id_str) {
                        // Try LAN direct delivery first
                        if let Some(addr) = target_addr {
                            match send_direct_message(&addr, &my_peer_id, &peer_id, message).await {
                                Ok(_) => {
                                    app.add_system_message(format!(
                                        "✅ Sent to @{} via LAN",
                                        target_username
                                    ));
                                    app.add_message(
                                        my_peer_id.to_string(),
                                        format!("[→ @{}] {}", target_username, message),
                                    );
                                    continue;
                                }
                                Err(e) => {
                                    app.add_system_message(format!(
                                        "⚠️ LAN send failed: {}, trying beacon...",
                                        e
                                    ));
                                }
                            }
                        }

                        // Fallback to beacon
                        if let Some(ref mut client) = beacon_sender {
                            let payload = format!("TO:{}:{}", target_username, message);
                            let packet =
                                Packet::message(&my_peer_id, &my_peer_id, payload.as_bytes());
                            match client.send_packet(&packet).await {
                                Ok(_) => {
                                    app.add_system_message(format!(
                                        "✅ Sent to @{} via beacon",
                                        target_username
                                    ));
                                    app.add_message(
                                        my_peer_id.to_string(),
                                        format!("[→ @{}] {}", target_username, message),
                                    );
                                }
                                Err(e) => {
                                    app.add_system_message(format!("❌ Send failed: {}", e));
                                }
                            }
                        } else {
                            app.add_system_message(
                                "❌ No beacon connection and LAN delivery failed".to_string(),
                            );
                        }
                    } else {
                        app.add_system_message(format!(
                            "❌ Invalid PeerId for user '{}'",
                            target_username
                        ));
                    }
                } else {
                    app.add_system_message(format!("❌ User '{}' not found", target_username));
                }
            } else {
                app.add_system_message("Usage: /msg <username> <message>".to_string());
            }
        } else if input.starts_with("/send ") {
            let parts: Vec<&str> = input[6..].splitn(2, ' ').collect();
            if parts.len() == 2 {
                match PeerId::from_hex(parts[0]) {
                    Ok(recipient) => {
                        let recipient_str = recipient.to_string();
                        let mut sent = false;

                        // Try LAN first
                        for (id, addr) in &lan_peers {
                            if id == &recipient_str {
                                match send_direct_message(addr, &my_peer_id, &recipient, parts[1])
                                    .await
                                {
                                    Ok(_) => {
                                        app.add_system_message(format!(
                                            "✅ Sent to {} via LAN",
                                            recipient.short()
                                        ));
                                        app.add_message(
                                            my_peer_id.to_string(),
                                            format!("[→ {}] {}", recipient.short(), parts[1]),
                                        );
                                        sent = true;
                                        break;
                                    }
                                    Err(e) => {
                                        app.add_system_message(format!(
                                            "⚠️ LAN send failed: {}",
                                            e
                                        ));
                                    }
                                }
                            }
                        }

                        // Fallback to beacon
                        if !sent {
                            if let Some(ref mut client) = beacon_sender {
                                let packet =
                                    Packet::message(&my_peer_id, &recipient, parts[1].as_bytes());
                                match client.send_packet(&packet).await {
                                    Ok(_) => {
                                        app.add_system_message(format!(
                                            "✅ Sent to {} via beacon",
                                            recipient.short()
                                        ));
                                        app.add_message(
                                            my_peer_id.to_string(),
                                            format!("[→ {}] {}", recipient.short(), parts[1]),
                                        );
                                    }
                                    Err(e) => {
                                        app.add_system_message(format!("❌ Send failed: {}", e));
                                    }
                                }
                            } else {
                                app.add_system_message(
                                    "❌ No beacon connection and LAN delivery failed".to_string(),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        app.add_system_message(format!("❌ Invalid PeerId: {}", e));
                    }
                }
            } else {
                app.add_system_message("Usage: /send <peer_id> <message>".to_string());
            }
        } else if input == "/nat" {
            let nat = NatDetector::detect(port).await;
            app.add_system_message(format!(
                "NAT: {:?} | External: {:?}",
                nat.nat_type, nat.external_addr
            ));
        } else if input.starts_with("/ping ") {
            let target = input[6..].trim();
            match send_ping(target).await {
                Ok(_) => app.add_system_message(format!("✅ Ping OK: {}", target)),
                Err(e) => app.add_system_message(format!("❌ Ping failed: {}", e)),
            }
        } else if input == "/lan" {
            app.add_system_message(format!("🏠 LAN peers: {}", lan_peers.len()));
            for (id, addr) in &lan_peers {
                app.add_system_message(format!("  • {} at {}", &id[..8], addr));
            }
        } else if !input.starts_with('/') {
            app.add_system_message(format!("❌ Unknown command: {}", input));
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

/// Send a direct message via TCP (LAN)
async fn send_direct_message(
    addr: &str,
    from: &PeerId,
    to: &PeerId,
    message: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(addr).await?;

    let packet = Packet::message(from, to, message.as_bytes());
    let bytes = packet.serialize();

    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&bytes).await?;

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

    println!("📡 TCP server listening on {}", addr);

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
                                            "💬 [{}] Direct message from {}: {}",
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

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
// use undertow_protocol::beacon::beacon_client::BeaconClient;
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
    // ВАЖНО: Используйте ФИКСИРОВАННЫЙ порт для UDP multicast (например, 9003),
    // иначе узлы на разных TCP-портах (9001 и 9002) не услышат друг друга!
    let lan_discovery_port = 9003;

    let lan_discovery =
        LanDiscovery::new(my_peer_id.clone(), username.clone(), lan_discovery_port).await?;

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let lan_discovery_clone = lan_discovery.clone();
    // Приводим к нижнему регистру для безопасного сравнения с приходящими данными
    let my_peer_id_hex = my_peer_id.to_string().to_lowercase();
    let tx_lan = tx.clone();

    tokio::spawn(async move {
        if let Err(e) = lan_discovery_clone
            .listen(move |msg, addr| {
                if msg.peer_id.to_lowercase() == my_peer_id_hex {
                    return; // Игнорируем собственные анонсы
                }
                match msg.msg_type {
                    LanMessageType::Announce => {
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
            .await
        {
            eprintln!("❌ [LAN] Listen task failed: {}", e);
        }
    });

    let lan_announce = lan_discovery.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let _ = lan_announce.announce().await;
        }
    });
    let _ = lan_discovery.announce().await;
    println!(
        "🏠 LAN Discovery started on UDP port {} (announcing every 10s)",
        lan_discovery_port
    );

    // === PHASE 4: TUI Setup ============================================
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let local_addr_strs: Vec<String> = local_addrs.iter().map(|a| a.to_string()).collect();

    // Создаём App ОДИН раз перед циклом
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
        my_peer_id.to_string()
    ));
    app.add_system_message("🏠 LAN mode: searching for peers in local network...".to_string());

    // Храним пиры надёжно: peer_id -> (username, addr)
    let mut lan_peers: HashMap<String, (String, String)> = HashMap::new();

    // === PHASE 6: Main TUI Loop =========================================
    loop {
        // 1. Обрабатываем ВСЕ накопившиеся сетевые события и обновляем `app`
        while let Ok(msg) = rx.try_recv() {
            if msg.starts_with("LAN_PEER_FOUND:") {
                let parts: Vec<&str> = msg[15..].splitn(3, ':').collect();
                if parts.len() == 3 {
                    let peer_id = parts[0].to_string();
                    let peer_username = parts[1].to_string();
                    let addr = parts[2].to_string();

                    lan_peers.insert(peer_id.clone(), (peer_username.clone(), addr.clone()));

                    if !app
                        .connected_peers_info
                        .iter()
                        .any(|(id, _)| id == &peer_id)
                    {
                        app.add_peer(peer_id.clone(), peer_username.clone());
                        let short_id = &peer_id[..8.min(peer_id.len())];
                        app.add_system_message(format!(
                            "🏠 LAN peer found: {} ({}) at {}",
                            peer_username, short_id, addr
                        ));
                    }
                }
            } else if msg.starts_with("LAN_PEER_LEFT:") {
                let parts: Vec<&str> = msg[14..].splitn(2, ':').collect();
                if parts.len() == 2 {
                    let peer_id = parts[0].to_string();
                    let peer_username = parts[1].to_string();

                    lan_peers.remove(&peer_id);
                    app.remove_peer(&peer_id);
                    let short_id = &peer_id[..8.min(peer_id.len())];
                    app.add_system_message(format!(
                        "🏠 LAN peer left: {} ({})",
                        peer_username, short_id
                    ));
                }
            } else if msg.starts_with("ERR:") {
                app.add_system_message(msg[4..].to_string());
            }
            // (Здесь можно оставить обработку MSG: для Beacon, если она нужна)
        }

        // Синхронизируем счётчик перед отрисовкой
        app.set_lan_peer_count(lan_peers.len());

        // 2. Отрисовываем UI. `app` передаётся по ссылке и сохраняет состояние!
        let input_opt = run_app(&mut terminal, &mut app)?;

        // 3. Обрабатываем ввод пользователя, НЕ уничтожая `app`
        if let Some(input) = input_opt {
            if input == "/quit" {
                let _ = lan_discovery.leave().await;
                break;
            }

            if input == "/help" || input == "help" {
                app.show_help = true;
            } else if input == "/peers" || input == "/lan" {
                if lan_peers.is_empty() {
                    app.add_system_message("🏠 No LAN peers found".to_string());
                } else {
                    app.add_system_message(format!("🏠 Active LAN peers: {}", lan_peers.len()));
                    for (peer_id, (peer_username, addr)) in &lan_peers {
                        let short_id = &peer_id[..8.min(peer_id.len())];
                        app.add_system_message(format!(
                            "  • {} ({}) at {}",
                            peer_username, short_id, addr
                        ));
                    }
                }
            } else if !input.starts_with('/') {
                // Здесь должен быть вызов отправки сообщения, если ваш LanDiscovery это поддерживает
                // Например: let _ = lan_discovery.broadcast_message(&input).await;
                app.add_system_message(
                    "⚠️ LAN broadcast messaging not yet implemented in this stub".to_string(),
                );
                app.add_message(my_peer_id.to_string(), input.clone());
            } else {
                app.add_system_message(format!("❌ Unknown command: {}", input));
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

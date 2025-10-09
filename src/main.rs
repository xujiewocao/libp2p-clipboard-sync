use clap::Parser;
use futures::StreamExt;
use anyhow::Result;
use log::{debug, error, info};
use tokio::{io, io::AsyncBufReadExt, select};
use std::{
    collections::hash_map::DefaultHasher, 
    error::Error, 
    hash::{Hash, Hasher}, 
    net::IpAddr, 
    time::Duration,
};
use libp2p::{
    gossipsub, identify, identity, 
    mdns, noise, swarm::{NetworkBehaviour, SwarmEvent}, 
    tcp, yamux, 
    multiaddr::{Multiaddr, Protocol}, 
    PeerId, Swarm, SwarmBuilder
};

// Default ports
const PORT_TCP: u16 = 0;  // 0 means OS will assign a random available port
const CHAT_TOPIC: &str = "libp2p-chat";
const CLIPBOARD_TOPIC: &str = "libp2p-clipboard";

#[derive(NetworkBehaviour)]
struct AppBehaviour {
    identify: identify::Behaviour,
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[derive(Parser, Debug)]
#[clap(name = "libp2p app", version = "1.0", author = "Eric Xu")]
struct Args {
    /// Address to listen on
    #[clap(long, default_value = "0.0.0.0")]
    listen_address: IpAddr,

    /// Nodes to connect to on startup
    #[clap(long)]
    connect: Option<Vec<Multiaddr>>,
    
    /// Enable clipboard sync
    #[clap(long)]
    clipboard: bool,
}

mod clipboard;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local peer id: {:?}", local_peer_id);

    // Create the swarm
    let mut swarm = create_swarm(local_key)?;

    // Create a Gossipsub topic and subscribe to it
    let chat_topic = gossipsub::IdentTopic::new(CHAT_TOPIC);
    swarm.behaviour_mut().gossipsub.subscribe(&chat_topic)
        .map_err(|e| anyhow::anyhow!("Failed to subscribe to chat topic: {:?}", e))?;
    
    // Subscribe to clipboard topic if enabled
    let clipboard_topic = if args.clipboard {
        let topic = gossipsub::IdentTopic::new(CLIPBOARD_TOPIC);
        swarm.behaviour_mut().gossipsub.subscribe(&topic)
            .map_err(|e| anyhow::anyhow!("Failed to subscribe to clipboard topic: {:?}", e))?;
        info!("Clipboard sync enabled");
        Some(topic)
    } else {
        None
    };

    // Build listening addresses
    let tcp_address = Multiaddr::from(args.listen_address)
        .with(Protocol::Tcp(PORT_TCP));

    // Start listening on the addresses
    swarm.listen_on(tcp_address.clone())
        .map_err(|e| anyhow::anyhow!("Failed to listen on TCP address: {:?}", e))?;
    info!("Listening on TCP: {}", tcp_address);

    // Connect to specified peers
    if let Some(addrs) = args.connect {
        for addr in addrs {
            info!("Dialing {addr}...");
            if let Err(e) = swarm.dial(addr.clone()) {
                error!("Failed to dial {addr}: {e}");
            }
        }
    }

    // Initialize clipboard sync if enabled
    let mut clipboard_rx = None;
    let clipboard_sync = clipboard::ClipboardSync::new().expect("Failed to create clipboard sync");
    if args.clipboard {
        // Create a channel for clipboard content
        let (clipboard_tx, rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        clipboard_rx = Some(rx);
        
        let clipboard_sync_clone = clipboard_sync.clone();

        // Start clipboard monitoring in a separate task
        if let Some(ref _clipboard_topic) = clipboard_topic {
            let clipboard_tx_clone = clipboard_tx.clone();
            
            tokio::spawn(async move {
                let clipboard = clipboard_sync_clone.clone();
                
                // Start monitoring clipboard changes
                clipboard.start_monitoring(move |content| {
                    // Convert content to bytes for network transmission
                    if let Ok(data) = serde_json::to_vec(&content) {
                        // Send clipboard content to the main thread for network transmission
                        let _ = clipboard_tx_clone.send(data);
                    }
                }).await.expect("Failed to start clipboard monitoring");
            });
        }
    }

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    // Main event loop
    info!("Enter messages to send to peers. Press Ctrl+C to exit.");
    loop {
        select! {
            // Handle user input from stdin
            Ok(Some(line)) = stdin.next_line() => {
                if !line.is_empty() {
                    // Check if there are peers subscribed to the topic before publishing
                    let peers = swarm.behaviour().gossipsub.all_peers().count();
                    if peers > 0 {
                        if let Err(e) = swarm
                            .behaviour_mut().gossipsub
                            .publish(chat_topic.clone(), line.as_bytes()) {
                            error!("Failed to publish message: {e:?}");
                        } else {
                            info!("Sent: {}", line);
                        }
                    } else {
                        // If no peers are connected, just echo the message locally
                        info!("[Local] {}", line);
                        info!("Note: No peers connected. Message not broadcast.");
                    }
                }
            }
            
            // Handle clipboard content to be sent
            Some(data) = async {
                if let Some(ref mut rx) = clipboard_rx {
                    rx.recv().await
                } else {
                    futures::future::pending().await
                }
            } => {
                // Send clipboard content to network
                if let Some(ref clipboard_topic) = clipboard_topic {
                    // Check if there are peers subscribed to the clipboard topic
                    let clipboard_peers = swarm.behaviour().gossipsub.all_peers()
                        .filter(|(_, topics)| topics.iter().any(|t| **t == clipboard_topic.hash()))
                        .count();
                    
                    if clipboard_peers > 0 {
                        if let Err(e) = swarm.behaviour_mut().gossipsub.publish(clipboard_topic.clone(), data) {
                            error!("Failed to publish clipboard content: {:?}", e);
                        } else {
                            info!("Clipboard content published to {} peers", clipboard_peers);
                        }
                    } else {
                        println!("No peers subscribed to clipboard topic. Content not published.\n");
                    }
                }
            }
            
            // Handle swarm events
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Local node is listening on {address}");
                },
                
                // Identify events
                SwarmEvent::Behaviour(AppBehaviourEvent::Identify(identify::Event::Sent { peer_id, .. })) => {
                    info!("Sent identify info to {peer_id:?}")
                }
                SwarmEvent::Behaviour(AppBehaviourEvent::Identify(identify::Event::Received { info, .. })) => {
                    info!("Received identify info from peer: {info:?}")
                },
                
                // mDNS events
                SwarmEvent::Behaviour(AppBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        info!("mDNS discovered a new peer: {peer_id} at {multiaddr}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(AppBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        info!("mDNS peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },
                
                // Gossipsub events
                SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: _id,
                    message,
                })) => {
                    // Check which topic the message is from by comparing with our subscribed topics
                    // For chat messages
                    if message.topic == chat_topic.hash() {
                        // Chat message
                        if let Ok(text) = String::from_utf8(message.data) {
                            info!("Received message from {}: {}", peer_id, text);
                        }
                    } 
                    // For clipboard messages
                    else if let Some(ref clipboard_topic) = clipboard_topic {
                        if message.topic == clipboard_topic.hash() {
                            // Handle clipboard message
                            if let Ok(content) = serde_json::from_slice::<clipboard::ClipboardContent>(&message.data) {
                                // Handle clipboard content in a separate task
                                let clipboard = clipboard_sync.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = clipboard.handle_incoming_content(content).await {
                                        error!("Failed to handle incoming clipboard content: {:?}", e);
                                    }
                                });
                            }
                        }
                    }
                },
                
                SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic })) => {
                    info!("Peer {peer_id} subscribed to topic {topic}");
                }
                
                // Connection events
                SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    info!("Connected to: {:?}", peer_id);
                    debug!("Endpoint: {:?}", endpoint);
                    // Add peer to gossipsub when connection is established
                    swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                },
                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    info!("Disconnected from: {:?}, cause: {:?}", peer_id, cause);
                    // Remove peer from gossipsub when connection is closed
                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                },
                
                _ => {}
            }
        }
    }
}

fn create_swarm(local_key: identity::Keypair) -> Result<Swarm<AppBehaviour>> {
    let local_peer_id = PeerId::from(local_key.public());
    debug!("Creating swarm for local peer id: {local_peer_id}");

    // Configure Gossipsub
    let message_id_fn = |message: &gossipsub::Message| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        gossipsub::MessageId::from(s.finish().to_string())
    };

    // Increase the max transmit size to support image transfers (10MB)
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .max_transmit_size(100 * 1024 * 1024) // 100MB max message size
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build gossipsub config: {:?}", e))?;

    let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    ).map_err(|e| anyhow::anyhow!("Failed to create gossipsub behaviour: {:?}", e))?;

    // Configure Identify
    let identify = identify::Behaviour::new(
        identify::Config::new("/ipfs/0.1.0".into(), local_key.public())
    );

    // Configure mDNS
    let mdns = mdns::tokio::Behaviour::new(
        mdns::Config::default(), 
        local_key.public().to_peer_id()
    ).map_err(|e| anyhow::anyhow!("Failed to create mdns behaviour: {:?}", e))?;

    // Create the behaviour
    let behaviour = AppBehaviour {
        gossipsub,
        identify,
        mdns
    };

    // Build the swarm
    let swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(), 
            noise::Config::new, 
            yamux::Config::default
        )?
        .with_behaviour(|_| behaviour)?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60))) 
        .build();

    Ok(swarm)
}
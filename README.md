# libp2p Clipboard Sync

A peer-to-peer decentralized clipboard synchronization application built with Rust and libp2p. This application allows multiple users to share clipboard content (text and images) in real-time across a local network.

## Features

- Automatic peer discovery on local networks using mDNS
- Secure communication with Noise encryption
- Real-time clipboard synchronization between peers
- Support for both text and image clipboard content
- Simple command-line interface
- Graceful handling of disconnected peers

## Prerequisites

- Rust and Cargo installed (https://www.rust-lang.org/tools/install)

## Building

```bash
cargo build
```

## Running

### Basic usage

Open multiple terminal windows and run the application in each:

```bash
cargo run
```

The first time you run the application, it will:
1. Generate a new identity
2. Start listening on a random TCP port
3. Begin discovering other peers on the local network via mDNS

### Enable clipboard synchronization

To enable clipboard synchronization between peers:

```bash
cargo run -- --clipboard
```

### Connecting to specific peers

You can also connect to specific peers using their multiaddresses:

```bash
# On Windows with CMD or PowerShell:
cargo run -- --connect /ip4/192.168.1.100/tcp/12345

# On Windows with Git Bash (note the double slashes):
cargo run -- --connect //ip4/192.168.1.100/tcp/12345

# Alternative for Git Bash:
cargo run -- "--connect" "\/ip4/192.168.1.100/tcp/12345"
```

### Specifying listen address

You can specify which address to listen on:

```bash
cargo run -- --listen-address 192.168.1.100
```

## Usage

1. Run the application in at least two terminal windows with the `--clipboard` flag
2. Wait a few seconds for mDNS discovery to find other peers
3. Copy text to the clipboard on one machine (Ctrl+C)
4. The content will be automatically synchronized to other machines
5. Paste the content on any other machine using standard paste (Ctrl+V)

## Error Handling

### NoPeersSubscribedToTopic Error

If you see this error when sending messages, it means there are no other peers connected to the chat network. This is normal when:

1. You're running only one instance of the application
2. Other instances haven't discovered each other yet (wait a few seconds)
3. Network issues prevent peer discovery

The application now handles this gracefully by echoing your messages locally with a note that they weren't broadcast.

### Git Bash Path Issue on Windows

When using Git Bash on Windows, you might encounter an error like:
```
error: invalid value 'C:/Program Files/Git/ip4/172.29.88.251/tcp/55984' for '--connect <CONNECT>': invalid multiaddr
```

This is a known issue with Git Bash path handling. Use one of these solutions:
1. Use double slashes: `cargo run -- --connect //ip4/172.29.88.251/tcp/55984`
2. Use PowerShell or CMD instead of Git Bash
3. Escape the path: `cargo run -- "--connect" "\/ip4/172.29.88.251/tcp/55984"`

## How it works

1. **Peer Discovery**: Uses mDNS to automatically discover other peers on the local network
2. **Message Propagation**: Uses Gossipsub to efficiently propagate clipboard content to all peers
3. **Security**: Uses Noise protocol for encrypted communication
4. **Identity**: Uses the Identify protocol to exchange peer information
5. **Clipboard Monitoring**: Monitors the system clipboard for changes and broadcasts them to peers
6. **Clipboard Setting**: Receives clipboard content from peers and sets the local system clipboard

## Example Output

```
[2024-01-01T12:00:00Z INFO  libp2p_clipboard] Local peer id: PeerId("12D3KooW...")
[2024-01-01T12:00:00Z INFO  libp2p_clipboard] Listening on TCP: /ip4/0.0.0.0/tcp/54321
[2024-01-01T12:00:05Z INFO  libp2p_clipboard] mDNS discovered a new peer: PeerId("12D3KooW...")
[2024-01-01T12:00:10Z INFO  libp2p_clipboard] Starting clipboard monitoring...
[2024-01-01T12:00:15Z INFO  libp2p_clipboard] Received clipboard content: Text
[2024-01-01T12:00:15Z INFO  libp2p_clipboard] Setting clipboard text: Hello, world!
```

## Dependencies

- [libp2p](https://crates.io/crates/libp2p) - The main networking library
- [tokio](https://crates.io/crates/tokio) - Asynchronous runtime
- [clap](https://crates.io/crates/clap) - Command line argument parsing
- [env_logger](https://crates.io/crates/env_logger) - Logging
- [anyhow](https://crates.io/crates/anyhow) - Error handling
- [arboard](https://crates.io/crates/arboard) - Cross-platform clipboard library
- [serde](https://crates.io/crates/serde) - Serialization framework
- [serde_json](https://crates.io/crates/serde_json) - JSON serialization

## License

This project is licensed under the MIT License - see the LICENSE file for details.
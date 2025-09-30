# libp2p Chat Application

A peer-to-peer decentralized chat application built with Rust and libp2p. This application uses:

- **Gossipsub** for message propagation
- **mDNS** for peer discovery on local networks
- **Identify** protocol for exchanging peer information
- **TCP** and **Noise** for secure transport

## Features

- Automatic peer discovery on local networks using mDNS
- Secure communication with Noise encryption
- Real-time messaging between peers
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

### Connecting to specific peers

You can also connect to specific peers using their multiaddresses:

```bash
cargo run -- --connect /ip4/192.168.1.100/tcp/12345
```

### Specifying listen address

You can specify which address to listen on:

```bash
cargo run -- --listen-address 192.168.1.100
```

## Usage

1. Run the application in at least two terminal windows
2. Wait a few seconds for mDNS discovery to find other peers
3. Type messages in any terminal and press Enter
4. Messages will be broadcast to all connected peers
5. Press Ctrl+C to exit

## Error Handling

### NoPeersSubscribedToTopic Error

If you see this error when sending messages, it means there are no other peers connected to the chat network. This is normal when:

1. You're running only one instance of the application
2. Other instances haven't discovered each other yet (wait a few seconds)
3. Network issues prevent peer discovery

The application now handles this gracefully by echoing your messages locally with a note that they weren't broadcast.

## How it works

1. **Peer Discovery**: Uses mDNS to automatically discover other peers on the local network
2. **Message Propagation**: Uses Gossipsub to efficiently propagate messages to all peers
3. **Security**: Uses Noise protocol for encrypted communication
4. **Identity**: Uses the Identify protocol to exchange peer information

## Example Output

```
[2024-01-01T12:00:00Z INFO  libp2p_chat] Local peer id: PeerId("12D3KooW...")
[2024-01-01T12:00:00Z INFO  libp2p_chat] Listening on TCP: /ip4/0.0.0.0/tcp/54321
[2024-01-01T12:00:05Z INFO  libp2p_chat] mDNS discovered a new peer: PeerId("12D3KooW...")
[2024-01-01T12:00:10Z INFO  libp2p_chat] Enter messages to send to peers. Press Ctrl+C to exit.
Hello everyone!
[2024-01-01T12:00:15Z INFO  libp2p_chat] Sent: Hello everyone!
[2024-01-01T12:00:16Z INFO  libp2p_chat] Received message from PeerId("12D3KooW..."): Hello everyone!
```

## Dependencies

- [libp2p](https://crates.io/crates/libp2p) - The main networking library
- [tokio](https://crates.io/crates/tokio) - Asynchronous runtime
- [clap](https://crates.io/crates/clap) - Command line argument parsing
- [env_logger](https://crates.io/crates/env_logger) - Logging
- [anyhow](https://crates.io/crates/anyhow) - Error handling

## License

This project is licensed under the MIT License - see the LICENSE file for details.
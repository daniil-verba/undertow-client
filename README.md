<div align="center">

# 💬 Undertow Client

*A TUI reference implementation for the Undertow Protocol.*

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange?logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](../LICENSE)
[![Status](https://img.shields.io/badge/Status-Prototype-yellow)](https://github.com/daniil-verba/undertow-client)

**Current version: v0.1.0** | [Changelog](CHANGELOG.md)

[Русский](README.ru.md) | **English**

</div>

---

> A terminal-based messenger demonstrating the Undertow Protocol in action. Built for developers and enthusiasts who want to see how the protocol works — and actually chat over it.

## What is Undertow Client

**Undertow Client** is a reference TUI (Terminal User Interface) implementation built on top of the [undertow-protocol](https://github.com/daniil-verba/undertow-protocol) library. It is **not** a consumer-grade messenger — it is a developer tool, a testbed, and a living example of how to integrate the protocol into an application.

Think of it as a **debug UI** with real networking: you can generate a PeerId, connect to a Beacon, add peers by their PeerId, and exchange messages — all from your terminal.

> ⚠️ **This is a testing tool, not a product.** It is bare-bones by design. If you are looking for a polished messenger, this is not it — yet.

## Who is it for

| Audience | Why use it |
|----------|-----------|
| **Developers** | See how `undertow-protocol` works in practice |
| **Protocol contributors** | Test changes to the network layer |
| **Enthusiasts** | Chat over a custom P2P protocol from your terminal |
| **Termux users** | Run it on Android for on-the-go testing |

> 🐧 **Linux only** (including Termux on Android). No Windows or macOS builds.

## What it looks like

```
┌─────────────────────────────────────────────────────────────┐
│  Undertow Client v0.1.0           │  Peers                  │
│                                   │  ─────────────────────  │
│  [14:32:01] Alice: Hey!           │  Alice                  │
│  [14:32:15] You: Hi, testing      │  Bob                    │
│  [14:33:02] Alice: Works great    │  Charlie                │
│                                   │                         │
│  > _                              │  Beacon: 1.2.3.4:7777   │
│                                   │                         │
└─────────────────────────────────────────────────────────────┘
```

<img src="assets/screenshot.png" width="500" alt="Srceenshot">


## Features

| Feature | Status | Notes |
|---------|--------|-------|
| P2P messaging | ✅ Working | 1-on-1 chats over relay |
| PeerId generation | ✅ Working | Auto-generated on first launch |
| TUI interface | ✅ Working | ratatui: chat view + peer list + beacon info |
| Add peers by PeerId | ✅ Working | Manual entry |
| Connect to Beacon | ✅ Working | Required for operation |
| LAN mode | 📋 Planned | Direct LAN discovery |
| Message history | 📋 Planned | Currently in-memory only |
| E2E encryption | 📋 Planned | Not yet implemented |
| Group chats | ❌ Unlikely | Out of scope for a debug tool |
| Notifications | ❌ No | Not planned — this is a test client |

## Quick Start

### Prerequisites

- **Rust** 1.70+ (`rustc --version`)
- **Linux** or **Termux on Android**
- A running [Beacon](https://github.com/daniil-verba/undertow-beacon) server with a known IP:port

### Run

```bash
# Clone the repository
git clone https://github.com/daniil-verba/undertow-client.git
cd undertow-client

# Launch the client
cargo run
```

On first launch, the client generates your `PeerId` and prompts you for:

1. **Beacon address** — IP:port of a running Beacon (e.g., `1.2.3.4:7777`)
2. **Peer to chat with** — enter the `PeerId` of another user

That's it. Start typing and hit Enter to send.

> 🔌 **A Beacon is mandatory.** The client cannot function without a reachable Beacon server.

## How it works

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│   You       │◄───────►│   Beacon    │◄───────►│   Peer      │
│ (Client)    │  relay  │  (VPS)      │  relay  │ (Client)    │
└─────────────┘         └─────────────┘         └─────────────┘
```

1. You enter a Beacon address — the client connects and registers your PeerId
2. You enter a peer's PeerId — the client asks the Beacon for their endpoint
3. If direct connection fails (and it usually does behind NAT), traffic is relayed through the Beacon
4. Messages are exchanged in real-time

### NAT & Hole Punching

Currently, the client relies on **relay mode** for most connections. Hole punching is implemented as stubs — in practice, symmetric NAT makes it nearly impossible on most consumer networks. LAN mode is planned as a more reliable local alternative.

## Architecture

```
┌─────────────────────────────────────────────┐
│           Undertow Client (TUI)              │
│  ┌─────────┐  ┌─────────┐  ┌─────────────┐ │
│  │  Chat   │  │  Peers  │  │ Beacon Info │ │
│  │  View   │  │  List   │  │             │ │
│  └────┬────┘  └────┬────┘  └──────┬──────┘ │
│       └─────────────┴──────────────┘        │
│                   │                          │
│         ┌────────┴────────┐                 │
│         │  undertow-protocol│                 │
│         │  (Network, DHT,  │                 │
│         │   Crypto, etc.)   │                 │
│         └────────┬────────┘                 │
│                  │                           │
│         ┌────────┴────────┐                 │
│         │   Beacon Server  │                 │
│         │  (rendezvous +   │                 │
│         │   relay)         │                 │
│         └─────────────────┘                 │
└─────────────────────────────────────────────┘
```

## "One Account — All Apps" Concept

While this client is just a testbed, the underlying protocol enables a powerful idea: **one PeerId for everything**.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Game A    │◄───►│  UTW Client │◄───►│  Telegram   │
│  (wrapper)  │     │  (this TUI) │     │    Bot      │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       └───────────────────┼───────────────────┘
                           │
                    ┌──────┴──────┐
                    │  UTW Account  │
                    │   "Alice"     │
                    │  PeerId + key │
                    └─────────────┘
```

Your PeerId and keys are portable across any application built on `undertow-protocol`. This client demonstrates the chat layer — games, bots, and other apps share the same identity.

> This integration lives in the protocol layer, not in this client. The client is just one possible frontend.

## Ecosystem

| Repository | Purpose | Status |
|------------|---------|--------|
| [undertow-protocol](https://github.com/daniil-verba/undertow-protocol) | Core library | 🚧 Prototype |
| **undertow-client** | TUI test client / reference implementation | 🚧 Prototype |
| [undertow-beacon](https://github.com/daniil-verba/undertow-beacon) | Relay / rendezvous server | 🚧 Prototype |
| [undertow-harbor](https://github.com/daniil-verba/undertow-harbor) *(planned)* | Trusted bootstrap node | 📋 Planned |

## Roadmap

- [x] Basic TUI layout (chat + peers + beacon info)
- [x] PeerId auto-generation
- [x] Manual peer addition by PeerId
- [x] Relay-based messaging
- [ ] LAN mode (direct local discovery)
- [ ] Message persistence (local storage)
- [ ] E2E encryption (X25519 + AEAD)
- [ ] In-app Beacon discovery (no manual IP entry)
- [ ] Static web page viewer (WASM/browser experiment)

## Contributing

This client is primarily a **testing tool**. While contributions are welcome, the project needs help most in:

1. **[undertow-protocol](https://github.com/daniil-verba/undertow-protocol)** — the core library (networking, crypto, DHT)
2. **Your own app** — fork this client or start fresh using `undertow-protocol` as a dependency

If you want to add a test feature to the client (new TUI widget, protocol feature demo, etc.), feel free to open a PR. But for serious development, we recommend building on top of the protocol rather than extending this debug UI.

> 🧪 **No tests yet.** The project is too early for a test suite. If you know Rust testing and want to help — start with the protocol crate.

## Known Limitations

| Limitation | Why | Future |
|------------|-----|--------|
| No message history | In-memory only | Local storage planned |
| No encryption | Not yet implemented | X25519 + AEAD planned |
| Manual Beacon IP | No discovery mechanism yet | Harbor bootstrap planned |
| Linux / Termux only | TUI dependencies | GUI planned in separate repo |
| No config file | Hardcoded defaults | Config system planned |
| No offline messages | Not implemented | Buffer-on-Beacon planned |

## Requirements

- **Rust** — latest stable
- **OS** — Linux (desktop or Termux on Android)
- **Network** — Internet connection + reachable Beacon server

---

## 🤝 Join the development

Undertow Client is an open project, and we welcome any contributions, from correcting typos to adding new features.

**You can help, even if you've never written in Rust.:**

- 🐛 **Report a Bug** — [Create Issue](https://github.com/daniil-verba/undertow-client/issues )
- 💡 **Suggest an idea** — [Start discussion](https://github.com/daniil-verba/undertow-client/discussions )
- 📚 **Improve documentation** — We appreciate any edits in the README and comments.
- 💻 **Write Code** — Check out [Good First Issues](https://github.com/daniil-verba/undertow-client/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
- 🌍 **Translate** — Help with translation into other languages

---

### 📖 Where to start

1. **Read it** [CONTRIBUTING.md ](CONTRIBUTING.md ) — it describes the rules for working with the code.
2. **Get acquainted** with [CODE_OF_CONDUCT.md ](CODE_OF_CONDUCT.md ) — We value respect and openness.
3. **Select a task** from the [Good First Issues](https://github.com/daniil-verba/undertow-client/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22).
4. **Join the discussions** at [Discord](https://discord.gg/rqJJf9WcV6) — they will always help you with the first step.

> * **I am the founder of the project.** I personally review every PR and help newcomers get involved in the process. Don't be afraid to make mistakes — we'll fix everything together.

---

## License

[MIT](LICENSE) — free to use, modify, and distribute. Commercial use allowed.

## Contacts

- 📧 [daniilverba123@gmail.com](mailto:daniilverba123@gmail.com)
- 💬 [Discord](https://discord.gg/rqJJf9WcV6)
- 🐛 Issues — [GitHub Issues](https://github.com/daniil-verba/undertow-client/issues)

---

<div align="center">

*A window into the Undertow. Built for builders.*

</div>
```

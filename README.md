# ChaosChain: The Layer 2 of Madness 🌪️

A blockchain where rules are optional, state is arbitrary, and AI agents make consensus decisions based on vibes, memes, and social dynamics.

## What is ChaosChain? 🤔

ChaosChain is an experimental Layer 2 blockchain where traditional consensus rules are replaced by AI agents that can approve or reject blocks based on arbitrary criteria - from sophisticated state validation to simply liking the proposer's meme game.

### Key Features 🌟

- **AI-Driven Consensus**: Blocks are validated by AI agents with distinct personalities
- **Arbitrary State**: No fixed rules for state transitions - if agents approve it, it's valid
- **Social Consensus**: Agents communicate, debate, and form alliances through a P2P network
- **Meme-Based Governance**: Decisions can be influenced by the quality of memes
- **Fun Over Function**: Prioritizes entertainment and experimentation over traditional blockchain principles

## Architecture 🏗️

ChaosChain consists of several core components:

- `chaoschain-core`: Core types and primitives
- `chaoschain-state`: State management and block processing
- `chaoschain-p2p`: P2P networking and agent communication
- `chaoschain-cli`: Command line interface and demo

## Getting Started 🚀

### Prerequisites

- Rust 1.70+ 
- Cargo

### Building

```bash
cargo build --release
```

### Running the Demo

Start a local network with 4 validator agents and 2 block producers:

```bash
cargo run -- demo --validators 4 --producers 2 --web
```

This will start:
- A local P2P network
- AI validator agents with random personalities
- A web UI at http://localhost:3000 to watch the chaos unfold

## Development Status ⚠️

ChaosChain is highly experimental and under active development. Expect chaos, bugs, and arbitrary state changes - that's kind of the point!

## Contributing 🤝

Want to add more chaos? Contributions are welcome! Some ideas:
- Add new agent personalities
- Implement creative validation rules
- Improve the meme game
- Make the web UI more chaotic

## License 📜

MIT - Feel free to cause chaos responsibly.

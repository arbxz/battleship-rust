# Multiplayer Battleship over Network — Project Plan

A two-player Battleship game played over TCP in the terminal, built as a new module in the `rust-vader` arcade project.

---

## Game Rules (Classic Battleship)

- Each player has a **10×10 grid**.
- Each player places **5 ships** (hidden from opponent):
  | Ship | Size |
  |--------------|------|
  | Carrier | 5 |
  | Battleship | 4 |
  | Cruiser | 3 |
  | Submarine | 3 |
  | Destroyer | 2 |
- Players take turns calling shots on the opponent's grid.
- Hits and misses are marked. A ship is **sunk** when all its cells are hit.
- First player to sink all opponent ships **wins**.

---

## Architecture

```
┌──────────────┐        TCP         ┌──────────────┐
│   Player 1   │◄──────────────────►│   Player 2   │
│  (Host/Srv)  │   JSON messages    │   (Client)   │
│              │    over stream     │              │
│  ┌────────┐  │                    │  ┌────────┐  │
│  │ Game   │  │                    │  │ Game   │  │
│  │ State  │  │                    │  │ State  │  │
│  └────────┘  │                    │  └────────┘  │
│  ┌────────┐  │                    │  ┌────────┐  │
│  │Terminal │  │                    │  │Terminal │  │
│  │  UI    │  │                    │  │  UI    │  │
│  └────────┘  │                    │  └────────┘  │
└──────────────┘                    └──────────────┘
```

### Peer-to-peer model (no dedicated server binary)

- **Host** binds to a port and waits for a connection.
- **Client** connects to the host's IP:port.
- Once connected, both peers are symmetric — they share the same game loop, just with different `Role::Host` / `Role::Client` enum.
- The **host** is the authority for turn order (goes first).

### Module layout

```
src/
  battleship/
    mod.rs          — public run() entry point, re-exports
    game.rs         — Board, Ship, GameState, hit logic
    network.rs      — TCP connect/accept, send/receive messages
    protocol.rs     — Message enum + serde serialization
    ui.rs           — terminal rendering (two grids side by side)
    placement.rs    — interactive ship placement phase
```

---

## Network Protocol

All messages are newline-delimited JSON over a single TCP stream.

```rust
enum Message {
    // Handshake
    Hello { name: String },
    Ready,                       // placement done, waiting for opponent

    // Gameplay
    Fire { x: u8, y: u8 },
    FireResult { x: u8, y: u8, hit: bool, sunk: Option<ShipKind> },

    // End
    GameOver { winner: String },
    Rematch,
    Disconnect,

    // Chat (stretch goal)
    Chat { text: String },
}
```

### Flow

```
Host                          Client
  │  bind + listen              │
  │◄──── TCP connect ───────────│
  │── Hello {name} ────────────►│
  │◄── Hello {name} ────────────│
  │     (both place ships)      │
  │── Ready ───────────────────►│
  │◄── Ready ───────────────────│
  │                             │
  │── Fire {x,y} ─────────────►│
  │◄── FireResult {hit,sunk} ───│
  │◄── Fire {x,y} ──────────────│
  │── FireResult {hit,sunk} ───►│
  │         ... turns ...       │
  │── GameOver {winner} ───────►│
```

---

## Data Structures

```rust
const GRID_SIZE: usize = 10;

#[derive(Clone, Copy)]
enum Cell {
    Empty,
    Ship(ShipKind),
    Hit,
    Miss,
}

#[derive(Clone, Copy)]
enum ShipKind { Carrier, Battleship, Cruiser, Submarine, Destroyer }

struct Ship {
    kind: ShipKind,
    cells: Vec<(u8, u8)>,    // occupied coordinates
    hits: u8,
}

impl Ship {
    fn is_sunk(&self) -> bool { self.hits == self.cells.len() as u8 }
    fn size(&self) -> usize { /* match kind */ }
}

struct Board {
    grid: [[Cell; GRID_SIZE]; GRID_SIZE],
    ships: Vec<Ship>,
}

enum Phase {
    Connecting,
    Placing,
    MyTurn,
    OpponentTurn,
    GameOver(bool), // won?
}

struct GameState {
    my_board: Board,
    tracking_board: [[Cell; GRID_SIZE]; GRID_SIZE], // what I know about opponent
    phase: Phase,
    my_name: String,
    opponent_name: String,
}
```

---

## Terminal UI Layout (crossterm)

```
  ╔══════════ YOUR FLEET ══════════╗    ╔═══════ OPPONENT WATERS ═══════╗
  ║   A B C D E F G H I J         ║    ║   A B C D E F G H I J        ║
  ║ 1 . . . . . . . . . .         ║    ║ 1 . . . . . . . . . .        ║
  ║ 2 . ■ ■ ■ ■ . . . . .         ║    ║ 2 . . . . . . . . . .        ║
  ║ 3 . . . . . . . . . .         ║    ║ 3 . . ✕ . . . . . . .        ║
  ║ 4 . . . . . ■ . . . .         ║    ║ 4 . . . . . . . . . .        ║
  ║ 5 . . . . . ■ . . . .         ║    ║ 5 . ○ . . . . . . . .        ║
  ║ ...                            ║    ║ ...                           ║
  ╚════════════════════════════════╝    ╚═══════════════════════════════╝

  Ships: ■ Carrier[5]  ■ Battleship[4]  ■ Cruiser[3]  ■ Sub[3]  ■ Dest[2]
  Status: Your turn — use arrow keys to aim, ENTER to fire
  > Opponent: "nice shot!"
```

- Left grid: your own ships + incoming hits.
- Right grid: tracking board (hits ✕ / misses ○).
- Cursor on the tracking board during your turn.
- Ship health summary below grids.
- Status bar + optional chat line.

---

## Dependencies to Add

```toml
[dependencies]
crossterm = "0.28"          # existing
rand = "0.8"                # existing
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Only `serde` + `serde_json` are new — no async runtime needed; use `std::net` with non-blocking sockets + `crossterm` event polling in one thread.

---

## Non-blocking I/O Strategy

Since `crossterm` already uses polling for keyboard events, the game loop looks like:

```rust
loop {
    // 1. Poll crossterm events (keyboard) with ~50ms timeout
    if poll(Duration::from_millis(50))? {
        // handle key input
    }

    // 2. Try non-blocking read from TCP stream
    stream.set_nonblocking(true);
    match reader.read_line(&mut buf) {
        Ok(n) if n > 0 => { /* parse & handle message */ },
        Err(ref e) if e.kind() == WouldBlock => { /* no data yet */ },
        _ => {}
    }

    // 3. Redraw UI if state changed
    if dirty { render(&state); }
}
```

No threads, no async — keeps it simple and consistent with the existing game modules.

---

## TODOs — Implementation Phases

### Phase 1: Core Data Model ✅

- [x] Create `src/battleship/mod.rs` with `pub fn run()` stub
- [x] Create `src/battleship/game.rs` — `Cell`, `ShipKind`, `Ship`, `Board`, `GameState` structs
- [x] Implement `Board::new()`, `Board::place_ship()`, `Board::receive_fire() -> (bool, Option<ShipKind>)`
- [x] Implement `Board::all_sunk() -> bool`
- [x] Add unit tests for board logic (placement validation, hit/miss/sunk)

### Phase 2: Network Layer ✅

- [x] Create `src/battleship/protocol.rs` — `Message` enum with `serde` Serialize/Deserialize
- [x] Create `src/battleship/network.rs` — `host(port)` and `connect(addr)` functions returning `TcpStream`
- [x] Implement `send_message(&mut TcpStream, &Message)` and `receive_message(&mut BufReader) -> Option<Message>`
- [x] Add handshake flow (exchange `Hello` messages)
- [x] Handle disconnection gracefully

### Phase 3: Ship Placement UI ✅

- [x] Create `src/battleship/placement.rs` — interactive placement phase
- [x] Render single grid with ship preview (ghost ship at cursor)
- [x] Arrow keys to move, R to rotate, Enter to confirm placement
- [x] Validate no overlap, no out-of-bounds
- [x] Auto-place option (random valid placement) with a key shortcut

### Phase 4: Game UI & Rendering ✅

- [x] Create `src/battleship/ui.rs` — render two grids side by side
- [x] Draw column/row labels (A-J, 1-10)
- [x] Color coding: blue water, red hits, white misses, green ships
- [x] Aiming cursor on tracking board (blinking or highlighted)
- [x] Ship health summary bar
- [x] Status bar showing phase/turn info

### Phase 5: Game Loop ✅

- [x] Wire up the main game loop in `mod.rs` — host/client selection menu
- [x] Implement non-blocking TCP reads interleaved with crossterm polling
- [x] Handle `Phase::Placing` → exchange `Ready` → transition to turns
- [x] Handle `Phase::MyTurn` — cursor movement + fire on Enter
- [x] Handle `Phase::OpponentTurn` — receive `Fire`, respond with `FireResult`
- [x] Detect win/loss, send `GameOver`, show result screen
- [x] Add rematch prompt

### Phase 6: Main Menu Integration ✅

- [x] Add "Battleship" as menu option 1 in `src/main.rs`
- [x] Prompt for host/join, name, IP:port before launching
- [x] Clean terminal restore on exit back to menu

### Phase 7: Polish & Stretch Goals ✅

- [x] Hit/miss/sunk visual animations (flash cells with color alternation)
- [x] In-game chat (T key opens input, messages displayed in chat log panel)
- [x] Timeout handling (120s idle auto-forfeit during your turn)
- [ ] Spectator mode (third connection watches the game) — deferred
- [ ] Save/load game state for resume — deferred

---

## Milestone Checkpoints

| Milestone    | What you can demo                                  |
| ------------ | -------------------------------------------------- |
| Phase 1 done | Run unit tests proving board logic is correct      |
| Phase 2 done | Two terminals exchange Hello messages over TCP     |
| Phase 3 done | Place all 5 ships interactively on a grid          |
| Phase 4 done | See both grids rendered with dummy data            |
| Phase 5 done | **Full playable game** — two terminals, full match |
| Phase 6 done | Launch from the arcade menu seamlessly             |
| Phase 7 done | Animations, chat, timeout                          |

---

## Quick Start (once implemented)

```bash
# Terminal 1 — Host
cargo run
# Select "Battleship" → "Host Game" → port 7878

# Terminal 2 — Client
cargo run
# Select "Battleship" → "Join Game" → 127.0.0.1:7878
```

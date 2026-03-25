// battleship/mod.rs — Entry point for the Multiplayer Battleship game module.
// Orchestrates the full game flow: connection setup, ship placement,
// turn-based combat loop, and game-over/rematch handling.

pub mod game;
mod menu;
pub mod network;
pub mod placement;
pub mod protocol;
pub mod ui;

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Stylize},
    terminal::{self, ClearType},
};

use game::{Board, Cell, GameState, Phase, GRID_SIZE, IDLE_TIMEOUT_SECS};
use network::Connection;
use protocol::Message;

// ---------------------------------------------------------------------------
// Cursor state for the aiming reticle
// ---------------------------------------------------------------------------

/// Tracks the aiming cursor position on the tracking board.
struct AimCursor {
    x: u8,
    y: u8,
}

impl AimCursor {
    fn new() -> Self {
        AimCursor { x: 0, y: 0 }
    }

    fn move_up(&mut self) {
        if self.y > 0 {
            self.y -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.y < (GRID_SIZE as u8 - 1) {
            self.y += 1;
        }
    }

    fn move_left(&mut self) {
        if self.x > 0 {
            self.x -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.x < (GRID_SIZE as u8 - 1) {
            self.x += 1;
        }
    }

    fn pos(&self) -> (u8, u8) {
        (self.x, self.y)
    }
}

// ---------------------------------------------------------------------------
// Turn handlers — extracted from game_loop for clarity
// ---------------------------------------------------------------------------

/// Handle keyboard input during the player's turn.
/// Returns `Ok(true)` if the player pressed Esc to forfeit.
fn handle_my_turn_input(
    stdout: &mut io::Stdout,
    key_code: KeyCode,
    aim: &mut AimCursor,
    state: &mut GameState,
    conn: &mut Connection,
    status_msg: &mut String,
) -> io::Result<bool> {
    match key_code {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => aim.move_up(),
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => aim.move_down(),
        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => aim.move_left(),
        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => aim.move_right(),

        KeyCode::Enter | KeyCode::Char(' ') => {
            fire_at_target(stdout, aim, state, conn, status_msg)?;
        }

        KeyCode::Char('t') | KeyCode::Char('T') => {
            send_chat_message(stdout, state, conn)?;
        }

        KeyCode::Esc => {
            let _ = conn.send(&Message::Disconnect);
            return Ok(true);
        }
        _ => {}
    }
    Ok(false)
}

/// Fire a shot at the current cursor position and process the result.
fn fire_at_target(
    stdout: &mut io::Stdout,
    aim: &AimCursor,
    state: &mut GameState,
    conn: &mut Connection,
    status_msg: &mut String,
) -> io::Result<()> {
    let (cx, cy) = aim.pos();

    // Don't fire at a cell we've already shot
    if state.tracking_board[cy as usize][cx as usize] != Cell::Empty {
        *status_msg = "Already fired there!".to_string();
        return Ok(());
    }

    // Send the Fire message
    conn.send(&Message::Fire { x: cx, y: cy })?;

    // Wait for the FireResult response (blocking)
    match conn.recv()? {
        Some(Message::FireResult { x, y, hit, sunk }) => {
            if hit {
                state.tracking_board[y as usize][x as usize] = Cell::Hit;
                // Flash animation on the tracking board
                ui::flash_cell(stdout, x, y, true, true)?;
                *status_msg = match sunk {
                    Some(kind) => {
                        // Flash the whole sunk ship if we know the cells
                        // (we only know the hit cell, so just extra flash)
                        format!("HIT! You sunk their {}!", kind.name())
                    }
                    None => "HIT!".to_string(),
                };
            } else {
                state.tracking_board[y as usize][x as usize] = Cell::Miss;
                ui::flash_cell(stdout, x, y, false, true)?;
                *status_msg = "Miss.".to_string();
            }

            // Check if we just won
            if state.has_won() {
                conn.send(&Message::GameOver {
                    winner: state.my_name.clone(),
                })?;
                state.phase = Phase::GameOver(true);
            } else {
                state.phase = Phase::OpponentTurn;
            }
        }
        Some(Message::Disconnect) | None => {
            *status_msg = "Opponent disconnected!".to_string();
            state.phase = Phase::GameOver(true);
        }
        _ => {
            *status_msg = "Unexpected message from opponent.".to_string();
        }
    }
    Ok(())
}

/// Process a single incoming message during the opponent's turn.
fn handle_opponent_message(
    stdout: &mut io::Stdout,
    state: &mut GameState,
    conn: &mut Connection,
    status_msg: &mut String,
) -> io::Result<()> {
    match conn.try_recv()? {
        Some(Message::Fire { x, y }) => {
            let (hit, sunk) = state.my_board.receive_fire(x, y);
            conn.send(&Message::FireResult { x, y, hit, sunk })?;

            // Flash animation on our own board
            ui::flash_cell(stdout, x, y, hit, false)?;

            if hit {
                if let Some(ref kind) = sunk {
                    // Flash the whole sunk ship
                    if let Some(ship) = state.my_board.ships.iter().find(|s| s.kind == *kind && s.is_sunk()) {
                        let cells = ship.cells.clone();
                        ui::flash_sunk_ship(stdout, &cells, false)?;
                    }
                }
            }

            *status_msg = if hit {
                match sunk {
                    Some(kind) => format!("Opponent hit and sunk your {}!", kind.name()),
                    None => format!("Opponent hit at {}{}!", (b'A' + x) as char, y + 1),
                }
            } else {
                format!("Opponent missed at {}{}.", (b'A' + x) as char, y + 1)
            };

            if state.my_board.all_sunk() {
                state.phase = Phase::GameOver(false);
            } else {
                state.phase = Phase::MyTurn;
            }
        }
        Some(Message::GameOver { .. }) => {
            state.phase = Phase::GameOver(false);
        }
        Some(Message::Chat { text }) => {
            let sender = state.opponent_name.clone();
            state.push_chat(sender, text);
        }
        Some(Message::Disconnect) => {
            *status_msg = "Opponent disconnected!".to_string();
            state.phase = Phase::GameOver(true);
        }
        // None from try_recv means no data yet (WouldBlock) — expected
        None => {}
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Chat input
// ---------------------------------------------------------------------------

/// Open a chat input line at the bottom of the screen, send the message.
fn send_chat_message(
    stdout: &mut io::Stdout,
    state: &mut GameState,
    conn: &mut Connection,
) -> io::Result<()> {
    let prompt_y = game::GRID_SIZE as u16 + 22;
    let text = menu::read_input_line(stdout, prompt_y, "Chat: ")?;
    if text.is_empty() {
        return Ok(());
    }
    conn.send(&Message::Chat { text: text.clone() })?;
    let my_name = state.my_name.clone();
    state.push_chat(my_name, text);
    Ok(())
}

// ---------------------------------------------------------------------------
// Game loop — the core turn-based gameplay
// ---------------------------------------------------------------------------

/// Run the main game loop after connection is established and ships are placed.
/// Handles turn alternation, firing, receiving fire, and game-over detection.
fn game_loop(
    stdout: &mut io::Stdout,
    state: &mut GameState,
    conn: &mut Connection,
) -> io::Result<()> {
    let mut aim = AimCursor::new();
    let mut status_msg = String::new();
    let mut last_activity = Instant::now();

    loop {
        // -- Render --
        let cursor_pos = if state.phase == Phase::MyTurn {
            Some(aim.pos())
        } else {
            None
        };
        ui::render_game(stdout, state, cursor_pos, &status_msg)?;

        // -- Check for game over --
        if let Phase::GameOver(_) = &state.phase {
            return handle_game_over(stdout, state, conn);
        }

        // -- Idle timeout check (only during our turn) --
        if state.phase == Phase::MyTurn
            && last_activity.elapsed().as_secs() >= IDLE_TIMEOUT_SECS
        {
            status_msg = "Idle timeout — auto-forfeit!".to_string();
            let _ = conn.send(&Message::Disconnect);
            state.phase = Phase::GameOver(false);
            continue;
        }

        // -- Poll for keyboard input (50ms timeout) --
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                last_activity = Instant::now();

                match state.phase {
                    Phase::MyTurn => {
                        if handle_my_turn_input(
                            stdout,
                            key.code,
                            &mut aim,
                            state,
                            conn,
                            &mut status_msg,
                        )? {
                            return Ok(());
                        }
                    }
                    Phase::OpponentTurn => {
                        match key.code {
                            KeyCode::Char('t') | KeyCode::Char('T') => {
                                send_chat_message(stdout, state, conn)?;
                            }
                            KeyCode::Esc => {
                                let _ = conn.send(&Message::Disconnect);
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        // -- Non-blocking check for incoming messages --
        // During opponent turn: receive Fire, GameOver, Chat, Disconnect
        // During our turn: receive Chat messages
        match state.phase {
            Phase::OpponentTurn => {
                handle_opponent_message(stdout, state, conn, &mut status_msg)?;
                // Reset activity timer when turn switches to us
                if state.phase == Phase::MyTurn {
                    last_activity = Instant::now();
                }
            }
            Phase::MyTurn => {
                // Only handle Chat during our turn (Fire won't arrive)
                if let Some(Message::Chat { text }) = conn.try_recv()? {
                    let sender = state.opponent_name.clone();
                    state.push_chat(sender, text);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Game over screen — show result, offer rematch
// ---------------------------------------------------------------------------

/// Show the game-over screen and handle rematch flow.
fn handle_game_over(
    stdout: &mut io::Stdout,
    state: &mut GameState,
    conn: &mut Connection,
) -> io::Result<()> {
    // Render one final time
    let end_msg = match state.phase {
        Phase::GameOver(true) => "VICTORY! Press R for rematch, Esc to quit.",
        Phase::GameOver(false) => "DEFEAT. Press R for rematch, Esc to quit.",
        _ => "Game over. Press Esc to quit.",
    };
    ui::render_game(stdout, state, None, end_msg)?;

    // Wait for rematch or quit
    loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    // Send rematch request
                    conn.send(&Message::Rematch)?;

                    // Wait for opponent's response
                    execute!(stdout, cursor::MoveTo(4, 22))?;
                    write!(
                        stdout,
                        "{}",
                        "Waiting for opponent to accept rematch...".with(Color::Yellow)
                    )?;
                    stdout.flush()?;

                    match conn.recv()? {
                        Some(Message::Rematch) => {
                            // Both agreed — restart with fresh state but keep connection
                            state.my_board = Board::new();
                            state.tracking_board =
                                [[Cell::Empty; GRID_SIZE]; GRID_SIZE];
                            state.phase = Phase::Placing;
                            return Ok(());
                        }
                        Some(Message::Disconnect) | None => {
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                KeyCode::Esc => {
                    let _ = conn.send(&Message::Disconnect);
                    return Ok(());
                }
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Launch the Battleship game. Handles connection setup, ship placement,
/// and the main game loop. Can be called repeatedly for rematches.
pub fn run() -> io::Result<()> {
    let mut stdout = io::stdout();

    // -- Connection menu: host or join --
    let (mut conn, my_name, is_host) = match menu::connection_menu(&mut stdout)? {
        Some(result) => result,
        None => return Ok(()), // user pressed Esc
    };

    // -- Handshake: exchange names --
    let opponent_name = conn.handshake(&my_name)?;

    // -- Initialize game state --
    let mut state = GameState::new(my_name);
    state.opponent_name = opponent_name;
    state.phase = Phase::Placing;

    // -- Main game loop (supports rematch) --
    loop {
        // Ship placement phase
        state.phase = Phase::Placing;
        let placed = placement::run_placement(&mut stdout, &mut state.my_board)?;
        if !placed {
            let _ = conn.send(&Message::Disconnect);
            return Ok(());
        }

        // Signal that we're ready
        conn.send(&Message::Ready)?;

        // Wait for opponent to be ready
        execute!(stdout, terminal::Clear(ClearType::All))?;
        execute!(stdout, cursor::MoveTo(4, 10))?;
        write!(
            stdout,
            "{}",
            "Ships placed! Waiting for opponent to finish placing...".with(Color::Yellow)
        )?;
        stdout.flush()?;

        loop {
            match conn.recv()? {
                Some(Message::Ready) => break,
                Some(Message::Disconnect) => return Ok(()),
                None => return Ok(()),
                _ => continue, // ignore unexpected messages
            }
        }

        // Host goes first
        state.phase = if is_host {
            Phase::MyTurn
        } else {
            Phase::OpponentTurn
        };

        // Run the turn-based game loop
        game_loop(&mut stdout, &mut state, &mut conn)?;

        // If game ended without the rematch flag being set, exit
        if state.phase != Phase::Placing {
            break;
        }
        // Otherwise, the rematch handler already reset state.phase to Placing,
        // so we loop back to placement.
    }

    Ok(())
}

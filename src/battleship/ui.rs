// battleship/ui.rs — Terminal rendering for the Battleship game.
// Draws two 10×10 grids side by side: the player's own fleet (left)
// and the opponent tracking board (right), plus ship health, status bar,
// and an aiming cursor on the tracking board during the player's turn.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crossterm::{
    cursor,
    execute,
    style::{Color, Stylize},
    terminal::{self, ClearType},
};

use super::game::{Board, Cell, GameState, Phase, ShipKind, GRID_SIZE};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Column labels displayed above each grid (A through J).
const COL_LABELS: [char; GRID_SIZE] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J'];

/// Left grid starts at this terminal column.
const LEFT_GRID_X: u16 = 2;
/// Right grid starts at this terminal column (spaced for readability).
const RIGHT_GRID_X: u16 = 38;
/// Both grids start at this terminal row.
const GRID_Y: u16 = 3;

// ---------------------------------------------------------------------------
// Cell rendering helpers
// ---------------------------------------------------------------------------

/// Return the character and color for a cell on the player's own board.
/// Ships are visible here.
fn own_cell_style(cell: Cell) -> (char, Color) {
    match cell {
        Cell::Empty => ('·', Color::DarkBlue),
        Cell::Ship(_) => ('■', Color::Green),
        Cell::Hit => ('✕', Color::Red),
        Cell::Miss => ('○', Color::DarkGrey),
    }
}

/// Return the character and color for a cell on the tracking board.
/// Ships are never shown — only hits and misses from our shots.
fn tracking_cell_style(cell: Cell) -> (char, Color) {
    match cell {
        Cell::Empty => ('·', Color::DarkBlue),
        Cell::Ship(_) => ('·', Color::DarkBlue), // hidden from us
        Cell::Hit => ('✕', Color::Red),
        Cell::Miss => ('○', Color::White),
    }
}

// ---------------------------------------------------------------------------
// Grid drawing
// ---------------------------------------------------------------------------

/// Draw a single 10×10 grid at the given terminal position.
/// `style_fn` determines how each cell is rendered (own vs tracking).
/// If `cursor_pos` is Some, that cell gets a highlighted background.
fn draw_grid(
    stdout: &mut io::Stdout,
    grid: &[[Cell; GRID_SIZE]; GRID_SIZE],
    origin_x: u16,
    origin_y: u16,
    style_fn: fn(Cell) -> (char, Color),
    cursor_pos: Option<(u8, u8)>,
) -> io::Result<()> {
    // Column labels
    execute!(stdout, cursor::MoveTo(origin_x + 4, origin_y))?;
    for ch in &COL_LABELS {
        write!(stdout, "{} ", format!("{}", ch).with(Color::Yellow))?;
    }

    // Grid rows
    #[allow(clippy::needless_range_loop)]
    for row in 0..GRID_SIZE {
        execute!(
            stdout,
            cursor::MoveTo(origin_x, origin_y + 1 + row as u16)
        )?;
        // Row label (1-10, right-aligned)
        write!(
            stdout,
            "{}",
            format!("{:>3} ", row + 1).with(Color::Yellow)
        )?;

        for col in 0..GRID_SIZE {
            let (ch, color) = style_fn(grid[row][col]);

            // Highlight the cursor cell with a contrasting background
            let is_cursor = cursor_pos
                .is_some_and(|(cx, cy)| cx as usize == col && cy as usize == row);

            if is_cursor {
                write!(
                    stdout,
                    "{} ",
                    format!("{}", ch).with(Color::Black).on(Color::Cyan)
                )?;
            } else {
                write!(stdout, "{} ", format!("{}", ch).with(color))?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Grid titles
// ---------------------------------------------------------------------------

/// Draw a title centered above a grid.
fn draw_title(
    stdout: &mut io::Stdout,
    text: &str,
    origin_x: u16,
    y: u16,
    color: Color,
) -> io::Result<()> {
    execute!(stdout, cursor::MoveTo(origin_x, y))?;
    write!(stdout, "{}", text.with(color))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Ship health summary
// ---------------------------------------------------------------------------

/// Draw the ship health bar showing each ship's status.
/// Sunk ships are crossed out in dark grey, alive ships show remaining hits.
fn draw_ship_health(
    stdout: &mut io::Stdout,
    board: &Board,
    x: u16,
    y: u16,
) -> io::Result<()> {
    execute!(stdout, cursor::MoveTo(x, y))?;
    write!(stdout, "{}", "Fleet: ".with(Color::Cyan))?;

    // Show status for each expected ship kind
    for kind in &ShipKind::all() {
        // Find this ship on the board (if placed)
        let ship = board.ships.iter().find(|s| s.kind == *kind);

        let label = format!("{}[{}]", kind.name(), kind.size());
        match ship {
            Some(s) if s.is_sunk() => {
                // Sunk — strikethrough style (dim + crossed text)
                write!(stdout, "{} ", label.with(Color::DarkRed).crossed_out())?;
            }
            Some(s) => {
                // Alive — show remaining health
                let remaining = kind.size() - s.hits as usize;
                let health = format!("{}({}/{})", kind.name(), remaining, kind.size());
                write!(stdout, "{} ", health.with(Color::Green))?;
            }
            None => {
                // Not yet placed
                write!(stdout, "{} ", label.with(Color::DarkGrey))?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

/// Draw the status bar at the bottom showing the current phase and instructions.
fn draw_status(
    stdout: &mut io::Stdout,
    state: &GameState,
    x: u16,
    y: u16,
    status_msg: &str,
) -> io::Result<()> {
    // Phase indicator
    execute!(stdout, cursor::MoveTo(x, y))?;
    let phase_text = match &state.phase {
        Phase::Connecting => "Connecting...".with(Color::Yellow),
        Phase::Placing => "Place your ships".with(Color::Cyan),
        Phase::MyTurn => "YOUR TURN — Arrow keys to aim, Enter to fire".with(Color::Green),
        Phase::OpponentTurn => "Opponent's turn — waiting...".with(Color::Yellow),
        Phase::GameOver(won) => {
            if *won {
                "VICTORY! You sank the entire fleet!".with(Color::Green)
            } else {
                "DEFEAT — Your fleet has been destroyed.".with(Color::Red)
            }
        }
    };
    write!(stdout, "{}", phase_text)?;

    // Player names
    execute!(stdout, cursor::MoveTo(x, y + 1))?;
    write!(
        stdout,
        "{} vs {}",
        state.my_name.as_str().with(Color::Cyan),
        if state.opponent_name.is_empty() {
            "???".with(Color::DarkGrey)
        } else {
            state.opponent_name.as_str().with(Color::Magenta)
        }
    )?;

    // Extra status message (e.g. "Hit!", "Miss!", "You sunk their Destroyer!")
    if !status_msg.is_empty() {
        execute!(stdout, cursor::MoveTo(x, y + 3))?;
        write!(stdout, "{}", status_msg.with(Color::Yellow))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public: full game screen render
// ---------------------------------------------------------------------------

/// Render the complete game screen: both grids, titles, ship health, status, chat.
///
/// - `cursor_pos`: if `Some((x, y))`, highlights that cell on the tracking
///   board (used during `MyTurn` for aiming).
/// - `status_msg`: optional one-line message shown below the status bar.
pub fn render_game(
    stdout: &mut io::Stdout,
    state: &GameState,
    cursor_pos: Option<(u8, u8)>,
    status_msg: &str,
) -> io::Result<()> {
    execute!(stdout, terminal::Clear(ClearType::All))?;

    // -- Titles above each grid --
    draw_title(stdout, "══ YOUR FLEET ══", LEFT_GRID_X, GRID_Y - 1, Color::Cyan)?;
    draw_title(
        stdout,
        "══ OPPONENT WATERS ══",
        RIGHT_GRID_X,
        GRID_Y - 1,
        Color::Magenta,
    )?;

    // -- Left grid: our own board (ships visible) --
    draw_grid(
        stdout,
        &state.my_board.grid,
        LEFT_GRID_X,
        GRID_Y,
        own_cell_style,
        None, // no cursor on our own board
    )?;

    // -- Right grid: tracking board (only hits/misses visible) --
    draw_grid(
        stdout,
        &state.tracking_board,
        RIGHT_GRID_X,
        GRID_Y,
        tracking_cell_style,
        cursor_pos, // aiming cursor when it's our turn
    )?;

    // -- Ship health below the left grid --
    let health_y = GRID_Y + GRID_SIZE as u16 + 2;
    draw_ship_health(stdout, &state.my_board, LEFT_GRID_X, health_y)?;

    // -- Status bar below everything --
    let status_y = health_y + 2;
    draw_status(stdout, state, LEFT_GRID_X, status_y, status_msg)?;

    // -- Chat log to the right of the status area --
    draw_chat_log(stdout, state, RIGHT_GRID_X, status_y)?;

    // -- Controls hint at the very bottom --
    let controls_y = status_y + 5;
    execute!(stdout, cursor::MoveTo(LEFT_GRID_X, controls_y))?;
    write!(
        stdout,
        "{}",
        "Arrow keys: Aim  Enter: Fire  T: Chat  Esc: Quit"
            .with(Color::DarkGrey)
    )?;

    stdout.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Chat log rendering
// ---------------------------------------------------------------------------

/// Draw the most recent chat messages.
fn draw_chat_log(
    stdout: &mut io::Stdout,
    state: &GameState,
    x: u16,
    y: u16,
) -> io::Result<()> {
    if state.chat_log.is_empty() {
        return Ok(());
    }

    execute!(stdout, cursor::MoveTo(x, y))?;
    write!(stdout, "{}", "── Chat ──".with(Color::DarkGrey))?;

    for (i, (sender, text)) in state.chat_log.iter().enumerate() {
        execute!(stdout, cursor::MoveTo(x, y + 1 + i as u16))?;
        write!(
            stdout,
            "{}: {}",
            sender.as_str().with(Color::Cyan),
            text.as_str().with(Color::White)
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Flash animation — visual feedback for hits, misses, and sinks
// ---------------------------------------------------------------------------

/// Flash a cell on the given grid position to give visual feedback.
/// The cell alternates between two colors several times.
/// `grid_side`: `false` = left (own board), `true` = right (tracking board).
pub fn flash_cell(
    stdout: &mut io::Stdout,
    x: u8,
    y: u8,
    hit: bool,
    grid_side: bool, // false = own board, true = tracking board
) -> io::Result<()> {
    let origin_x = if grid_side { RIGHT_GRID_X } else { LEFT_GRID_X };
    let term_col = origin_x + 4 + (x as u16 * 2);
    let term_row = GRID_Y + 1 + y as u16;

    let (ch, color_a, color_b) = if hit {
        ('✕', Color::White, Color::Red)
    } else {
        ('○', Color::White, Color::DarkGrey)
    };

    for i in 0..6 {
        let color = if i % 2 == 0 { color_a } else { color_b };
        let bg = if i % 2 == 0 { Color::Red } else { Color::Reset };

        execute!(stdout, cursor::MoveTo(term_col, term_row))?;
        if hit && i % 2 == 0 {
            write!(stdout, "{}", format!("{}", ch).with(color).on(bg))?;
        } else {
            write!(stdout, "{}", format!("{}", ch).with(color))?;
        }
        stdout.flush()?;
        thread::sleep(Duration::from_millis(80));
    }

    // Final state
    execute!(stdout, cursor::MoveTo(term_col, term_row))?;
    let final_color = if hit { Color::Red } else { Color::DarkGrey };
    write!(stdout, "{}", format!("{}", ch).with(final_color))?;
    stdout.flush()?;

    Ok(())
}

/// Flash all cells of a sunk ship on the own board for dramatic effect.
pub fn flash_sunk_ship(
    stdout: &mut io::Stdout,
    cells: &[(u8, u8)],
    grid_side: bool,
) -> io::Result<()> {
    let origin_x = if grid_side { RIGHT_GRID_X } else { LEFT_GRID_X };

    for i in 0..6 {
        for &(cx, cy) in cells {
            let term_col = origin_x + 4 + (cx as u16 * 2);
            let term_row = GRID_Y + 1 + cy as u16;
            execute!(stdout, cursor::MoveTo(term_col, term_row))?;

            if i % 2 == 0 {
                write!(stdout, "{}", "✕".with(Color::Yellow).on(Color::DarkRed))?;
            } else {
                write!(stdout, "{}", "✕".with(Color::Red))?;
            }
        }
        stdout.flush()?;
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

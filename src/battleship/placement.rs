// battleship/placement.rs — Interactive ship placement phase.
// Renders a 10×10 grid where the player moves a cursor, rotates,
// and confirms placement of each ship one by one.
// Controls: Arrow keys/WASD = move, R = rotate, Enter/Space = place,
//           A = auto-place all remaining ships randomly.

use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Stylize},
    terminal::{self, ClearType},
};
use rand::Rng;

use super::game::{Board, Cell, Orientation, ShipKind, GRID_SIZE};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Column labels displayed above the grid (A through J).
const COL_LABELS: [char; GRID_SIZE] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J'];

/// Top-left corner of the grid rendering area (col offset in terminal).
const GRID_OFFSET_X: u16 = 4;
/// Top-left corner of the grid rendering area (row offset in terminal).
const GRID_OFFSET_Y: u16 = 3;

// ---------------------------------------------------------------------------
// Placement state — tracks cursor position and current ship orientation
// ---------------------------------------------------------------------------

/// Internal state for the placement phase UI.
struct PlacementState {
    /// Cursor column on the grid (0..9).
    cursor_x: u8,
    /// Cursor row on the grid (0..9).
    cursor_y: u8,
    /// Current orientation for the next ship to be placed.
    orientation: Orientation,
    /// Index into `ShipKind::all()` — which ship we're placing next.
    ship_index: usize,
    /// Status message shown at the bottom (feedback for invalid moves, etc.).
    status: String,
}

impl PlacementState {
    fn new() -> Self {
        PlacementState {
            cursor_x: 0,
            cursor_y: 0,
            orientation: Orientation::Horizontal,
            ship_index: 0,
            status: String::new(),
        }
    }

    /// Returns the ShipKind currently being placed, or None if all are done.
    fn current_ship(&self) -> Option<ShipKind> {
        let all = ShipKind::all();
        if self.ship_index < all.len() {
            Some(all[self.ship_index])
        } else {
            None
        }
    }

    /// Compute the cells that the ghost ship preview would occupy
    /// at the current cursor position & orientation.
    /// Returns None if any cell would be out of bounds.
    fn ghost_cells(&self) -> Option<Vec<(u8, u8)>> {
        let kind = self.current_ship()?;
        let size = kind.size();
        let cells: Vec<(u8, u8)> = (0..size)
            .map(|i| match self.orientation {
                Orientation::Horizontal => (self.cursor_x + i as u8, self.cursor_y),
                Orientation::Vertical => (self.cursor_x, self.cursor_y + i as u8),
            })
            .collect();

        // Check bounds
        for &(cx, cy) in &cells {
            if cx as usize >= GRID_SIZE || cy as usize >= GRID_SIZE {
                return None;
            }
        }
        Some(cells)
    }

    /// Check if the ghost ship overlaps any already-placed ship on the board.
    fn ghost_overlaps(&self, board: &Board) -> bool {
        if let Some(cells) = self.ghost_cells() {
            cells
                .iter()
                .any(|&(cx, cy)| board.grid[cy as usize][cx as usize] != Cell::Empty)
        } else {
            true // out of bounds counts as invalid
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Draw the full placement screen: title, grid, ghost preview, ship list, controls.
fn render(stdout: &mut io::Stdout, board: &Board, state: &PlacementState) -> io::Result<()> {
    execute!(stdout, terminal::Clear(ClearType::All))?;

    // -- Title --
    execute!(stdout, cursor::MoveTo(GRID_OFFSET_X, 1))?;
    write!(
        stdout,
        "{}",
        "╔══════════ PLACE YOUR SHIPS ══════════╗"
            .with(Color::Cyan)
    )?;

    // -- Column labels --
    execute!(stdout, cursor::MoveTo(GRID_OFFSET_X + 4, GRID_OFFSET_Y))?;
    for ch in &COL_LABELS {
        write!(stdout, "{} ", format!("{}", ch).with(Color::Yellow))?;
    }

    // Compute ghost cells and whether they're valid for coloring
    let ghost_cells = state.ghost_cells();
    let ghost_valid = !state.ghost_overlaps(board);

    // -- Grid rows --
    for row in 0..GRID_SIZE {
        // Row label (1-10)
        execute!(
            stdout,
            cursor::MoveTo(GRID_OFFSET_X, GRID_OFFSET_Y + 1 + row as u16)
        )?;
        write!(
            stdout,
            "{}",
            format!("{:>2} ", row + 1).with(Color::Yellow)
        )?;

        for col in 0..GRID_SIZE {
            let is_ghost = ghost_cells
                .as_ref()
                .is_some_and(|cells| cells.contains(&(col as u8, row as u8)));

            // Determine what character and color to draw
            let (ch, color) = if is_ghost {
                // Ghost preview — green if valid, red if invalid
                if ghost_valid {
                    ('█', Color::Green)
                } else {
                    ('█', Color::Red)
                }
            } else {
                match board.grid[row][col] {
                    Cell::Empty => ('·', Color::DarkBlue),
                    Cell::Ship(_) => ('■', Color::White),
                    Cell::Hit => ('✕', Color::Red),
                    Cell::Miss => ('○', Color::DarkGrey),
                }
            };

            write!(stdout, "{} ", format!("{}", ch).with(color))?;
        }
    }

    // -- Ship list (which ships are placed / which is next) --
    let all_ships = ShipKind::all();
    let list_y = GRID_OFFSET_Y + GRID_SIZE as u16 + 2;
    execute!(stdout, cursor::MoveTo(GRID_OFFSET_X, list_y))?;
    write!(stdout, "{}", "Ships: ".with(Color::Cyan))?;

    for (i, kind) in all_ships.iter().enumerate() {
        let label = format!("{}[{}]", kind.name(), kind.size());
        if i < state.ship_index {
            // Already placed — dim
            write!(stdout, "{} ", label.with(Color::DarkGrey))?;
        } else if i == state.ship_index {
            // Currently placing — highlighted
            write!(stdout, "{} ", label.with(Color::Green).bold())?;
        } else {
            // Not yet placed
            write!(stdout, "{} ", label.with(Color::White))?;
        }
    }

    // -- Orientation indicator --
    execute!(stdout, cursor::MoveTo(GRID_OFFSET_X, list_y + 1))?;
    let orient_str = match state.orientation {
        Orientation::Horizontal => "Horizontal →",
        Orientation::Vertical => "Vertical ↓",
    };
    write!(
        stdout,
        "Orientation: {}",
        orient_str.with(Color::Yellow)
    )?;

    // -- Controls --
    execute!(stdout, cursor::MoveTo(GRID_OFFSET_X, list_y + 3))?;
    write!(
        stdout,
        "{}",
        "Arrow keys/WASD: Move  R: Rotate  Enter/Space: Place  A: Auto-place  Esc: Quit"
            .with(Color::DarkGrey)
    )?;

    // -- Status message (errors, confirmations) --
    if !state.status.is_empty() {
        execute!(stdout, cursor::MoveTo(GRID_OFFSET_X, list_y + 5))?;
        write!(stdout, "{}", state.status.clone().with(Color::Yellow))?;
    }

    stdout.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Auto-placement — randomly places all remaining ships
// ---------------------------------------------------------------------------

/// Randomly place all remaining ships (from `state.ship_index` onward)
/// on the board. Tries random positions until each ship fits.
fn auto_place_remaining(board: &mut Board, state: &mut PlacementState) {
    let mut rng = rand::thread_rng();
    let all_ships = ShipKind::all();

    while state.ship_index < all_ships.len() {
        let kind = all_ships[state.ship_index];

        // Try random placements until one works (bounded to prevent infinite loops)
        for _ in 0..1000 {
            let x = rng.gen_range(0..GRID_SIZE as u8);
            let y = rng.gen_range(0..GRID_SIZE as u8);
            let orientation = if rng.gen_bool(0.5) {
                Orientation::Horizontal
            } else {
                Orientation::Vertical
            };

            if board.place_ship(kind, x, y, orientation).is_ok() {
                state.ship_index += 1;
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point — run the interactive placement loop
// ---------------------------------------------------------------------------

/// Run the ship placement phase. The player interactively places all 5 ships
/// on the board using the terminal UI. Returns `Ok(true)` when placement is
/// complete, or `Ok(false)` if the player pressed Esc to quit.
pub fn run_placement(stdout: &mut io::Stdout, board: &mut Board) -> io::Result<bool> {
    let mut state = PlacementState::new();

    loop {
        // Check if all ships have been placed
        if state.current_ship().is_none() {
            // All ships placed — brief confirmation
            state.status = "All ships placed! Ready for battle.".to_string();
            render(stdout, board, &state)?;
            // Short pause so the player can see the final board
            std::thread::sleep(Duration::from_millis(800));
            return Ok(true);
        }

        render(stdout, board, &state)?;

        // Wait for keyboard input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                // Only handle key press events (not release/repeat)
                if key_event.kind != KeyEventKind::Press {
                    continue;
                }

                // Clear previous status each keypress
                state.status.clear();

                match key_event.code {
                    // -- Movement --
                    KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => {
                        if state.cursor_y > 0 {
                            state.cursor_y -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => {
                        if state.cursor_y < (GRID_SIZE as u8 - 1) {
                            state.cursor_y += 1;
                        }
                    }
                    KeyCode::Left => {
                        if state.cursor_x > 0 {
                            state.cursor_x -= 1;
                        }
                    }
                    KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                        if state.cursor_x < (GRID_SIZE as u8 - 1) {
                            state.cursor_x += 1;
                        }
                    }

                    // -- Rotate --
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        state.orientation = state.orientation.toggle();
                    }

                    // -- Place ship --
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(kind) = state.current_ship() {
                            match board.place_ship(
                                kind,
                                state.cursor_x,
                                state.cursor_y,
                                state.orientation,
                            ) {
                                Ok(()) => {
                                    state.status =
                                        format!("{} placed!", kind.name());
                                    state.ship_index += 1;
                                    // Reset cursor to origin for next ship
                                    state.cursor_x = 0;
                                    state.cursor_y = 0;
                                }
                                Err(msg) => {
                                    state.status = format!("Cannot place: {}", msg);
                                }
                            }
                        }
                    }

                    // -- Auto-place all remaining --
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        auto_place_remaining(board, &mut state);
                        state.status = "All ships auto-placed!".to_string();
                    }

                    // -- Quit placement --
                    KeyCode::Esc => {
                        return Ok(false);
                    }

                    _ => {}
                }
            }
        }
    }
}

// battleship/game.rs — Core data model for the Battleship game.
// Contains the board grid, ship definitions, placement/firing logic,
// and the overall game state tracker.

use std::fmt;

/// Grid dimensions (10×10 classic Battleship board).
pub const GRID_SIZE: usize = 10;

// ---------------------------------------------------------------------------
// Cell — represents the state of a single grid square
// ---------------------------------------------------------------------------

/// A single cell on the board grid.
/// - `Empty`: open water, no ship.
/// - `Ship(ShipKind)`: occupied by part of a ship (hidden from opponent).
/// - `Hit`: a shot that struck a ship.
/// - `Miss`: a shot that hit open water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cell {
    Empty,
    Ship(ShipKind),
    Hit,
    Miss,
}

// ---------------------------------------------------------------------------
// ShipKind — the five standard Battleship vessels
// ---------------------------------------------------------------------------

/// The five ship types in classic Battleship, each with a fixed size.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ShipKind {
    Carrier,    // 5 cells
    Battleship, // 4 cells
    Cruiser,    // 3 cells
    Submarine,  // 3 cells
    Destroyer,  // 2 cells
}

impl ShipKind {
    /// Returns the length (number of cells) this ship occupies.
    pub fn size(self) -> usize {
        match self {
            ShipKind::Carrier => 5,
            ShipKind::Battleship => 4,
            ShipKind::Cruiser => 3,
            ShipKind::Submarine => 3,
            ShipKind::Destroyer => 2,
        }
    }

    /// Returns a human-readable name for display in the UI.
    pub fn name(self) -> &'static str {
        match self {
            ShipKind::Carrier => "Carrier",
            ShipKind::Battleship => "Battleship",
            ShipKind::Cruiser => "Cruiser",
            ShipKind::Submarine => "Submarine",
            ShipKind::Destroyer => "Destroyer",
        }
    }

    /// Returns all five ship kinds in placement order (largest first).
    pub fn all() -> [ShipKind; 5] {
        [
            ShipKind::Carrier,
            ShipKind::Battleship,
            ShipKind::Cruiser,
            ShipKind::Submarine,
            ShipKind::Destroyer,
        ]
    }
}

impl fmt::Display for ShipKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// Orientation — horizontal or vertical placement
// ---------------------------------------------------------------------------

/// Ship placement direction on the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    Horizontal, // extends rightward from origin
    Vertical,   // extends downward from origin
}

impl Orientation {
    /// Flip between horizontal and vertical.
    pub fn toggle(self) -> Self {
        match self {
            Orientation::Horizontal => Orientation::Vertical,
            Orientation::Vertical => Orientation::Horizontal,
        }
    }
}

// ---------------------------------------------------------------------------
// Ship — a placed ship with hit tracking
// ---------------------------------------------------------------------------

/// A ship placed on the board. Tracks which cells it occupies
/// and how many of those cells have been hit.
#[derive(Clone, Debug)]
pub struct Ship {
    pub kind: ShipKind,
    /// The grid coordinates (col, row) this ship occupies.
    pub cells: Vec<(u8, u8)>,
    /// Number of cells that have been hit so far.
    pub hits: u8,
}

impl Ship {
    /// Returns `true` when every cell of this ship has been hit.
    pub fn is_sunk(&self) -> bool {
        self.hits as usize == self.cells.len()
    }

    /// Check whether this ship occupies the given coordinate.
    pub fn occupies(&self, x: u8, y: u8) -> bool {
        self.cells.iter().any(|&(cx, cy)| cx == x && cy == y)
    }
}

// ---------------------------------------------------------------------------
// Board — the 10×10 grid plus placed ships
// ---------------------------------------------------------------------------

/// Represents one player's board: the grid of cells and the list
/// of ships that have been placed on it.
#[derive(Clone, Debug)]
pub struct Board {
    /// The 10×10 grid. Indexed as `grid[row][col]`.
    pub grid: [[Cell; GRID_SIZE]; GRID_SIZE],
    /// All ships placed on this board.
    pub ships: Vec<Ship>,
}

impl Board {
    /// Creates a new empty board — all cells are `Empty`, no ships placed.
    pub fn new() -> Self {
        Board {
            grid: [[Cell::Empty; GRID_SIZE]; GRID_SIZE],
            ships: Vec::new(),
        }
    }

    /// Attempts to place a ship on the board.
    ///
    /// - `kind`: which ship to place.
    /// - `x`, `y`: top-left origin (col, row), zero-indexed.
    /// - `orientation`: horizontal (extends right) or vertical (extends down).
    ///
    /// Returns `Ok(())` on success, or `Err(message)` if the placement
    /// is out of bounds or overlaps an existing ship.
    pub fn place_ship(
        &mut self,
        kind: ShipKind,
        x: u8,
        y: u8,
        orientation: Orientation,
    ) -> Result<(), &'static str> {
        let size = kind.size();

        // Compute all cells the ship would occupy
        let cells: Vec<(u8, u8)> = (0..size)
            .map(|i| match orientation {
                Orientation::Horizontal => (x + i as u8, y),
                Orientation::Vertical => (x, y + i as u8),
            })
            .collect();

        // Bounds check — every cell must be within the 10×10 grid
        for &(cx, cy) in &cells {
            if cx as usize >= GRID_SIZE || cy as usize >= GRID_SIZE {
                return Err("Ship placement is out of bounds");
            }
        }

        // Overlap check — every cell must be empty
        for &(cx, cy) in &cells {
            if self.grid[cy as usize][cx as usize] != Cell::Empty {
                return Err("Ship overlaps with an existing ship");
            }
        }

        // Place the ship on the grid
        for &(cx, cy) in &cells {
            self.grid[cy as usize][cx as usize] = Cell::Ship(kind);
        }

        // Record the ship for hit tracking
        self.ships.push(Ship {
            kind,
            cells,
            hits: 0,
        });

        Ok(())
    }

    /// Process an incoming shot at coordinates (x, y).
    ///
    /// Returns a tuple:
    /// - `bool`: `true` if a ship was hit, `false` for a miss.
    /// - `Option<ShipKind>`: `Some(kind)` if the hit just sunk a ship.
    ///
    /// The grid cell is updated to `Hit` or `Miss` accordingly.
    pub fn receive_fire(&mut self, x: u8, y: u8) -> (bool, Option<ShipKind>) {
        let row = y as usize;
        let col = x as usize;

        match self.grid[row][col] {
            Cell::Ship(kind) => {
                // Mark the cell as hit
                self.grid[row][col] = Cell::Hit;

                // Find the ship and increment its hit counter
                let mut sunk_kind = None;
                for ship in &mut self.ships {
                    if ship.kind == kind && ship.occupies(x, y) {
                        ship.hits += 1;
                        if ship.is_sunk() {
                            sunk_kind = Some(kind);
                        }
                        break;
                    }
                }

                (true, sunk_kind)
            }
            Cell::Empty => {
                // Miss — mark the cell
                self.grid[row][col] = Cell::Miss;
                (false, None)
            }
            // Already hit or missed — treat as a miss (no double-fire)
            _ => (false, None),
        }
    }

    /// Returns `true` when every ship on this board has been sunk.
    pub fn all_sunk(&self) -> bool {
        self.ships.iter().all(|ship| ship.is_sunk())
    }
}

impl Default for Board {
    fn default() -> Self {
        Board::new()
    }
}

// ---------------------------------------------------------------------------
// Phase — tracks which stage of the game we're in
// ---------------------------------------------------------------------------

/// The current phase of the game, driving what the UI shows
/// and what inputs are accepted.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Waiting for TCP connection to be established.
    Connecting,
    /// Both players are placing their ships.
    Placing,
    /// It's our turn to fire a shot.
    MyTurn,
    /// Waiting for the opponent to fire.
    OpponentTurn,
    /// Game is over. `bool` is `true` if we won.
    GameOver(bool),
}

// ---------------------------------------------------------------------------
// GameState — the full state for one player's view of the game
// ---------------------------------------------------------------------------

/// Maximum number of chat messages kept in the visible log.
const MAX_CHAT_LOG: usize = 5;

/// Idle timeout in seconds — auto-forfeit if a player doesn't act.
pub const IDLE_TIMEOUT_SECS: u64 = 120;

/// Top-level game state holding both boards, the current phase,
/// and player names.
#[derive(Clone, Debug)]
pub struct GameState {
    /// Our own board showing ship positions and incoming hits.
    pub my_board: Board,
    /// Tracking board: what we know about the opponent's grid
    /// (hits and misses from our shots). Ships are never revealed here.
    pub tracking_board: [[Cell; GRID_SIZE]; GRID_SIZE],
    /// Current game phase.
    pub phase: Phase,
    /// Our display name.
    pub my_name: String,
    /// Opponent's display name.
    pub opponent_name: String,
    /// Chat message log (most recent last), displayed on the game screen.
    pub chat_log: Vec<(String, String)>,
}

impl GameState {
    /// Creates a new game state with empty boards in the `Connecting` phase.
    pub fn new(my_name: String) -> Self {
        GameState {
            my_board: Board::new(),
            tracking_board: [[Cell::Empty; GRID_SIZE]; GRID_SIZE],
            phase: Phase::Connecting,
            my_name,
            opponent_name: String::new(),
            chat_log: Vec::new(),
        }
    }

    /// Add a chat message to the log, trimming old entries.
    pub fn push_chat(&mut self, sender: String, text: String) {
        self.chat_log.push((sender, text));
        if self.chat_log.len() > MAX_CHAT_LOG {
            self.chat_log.remove(0);
        }
    }

    /// Check whether the player has sunk all opponent ships by counting
    /// hits on the tracking board against the total number of ship cells.
    pub fn has_won(&self) -> bool {
        let total_ship_cells: usize = ShipKind::all().iter().map(|k| k.size()).sum();
        let hit_count = self
            .tracking_board
            .iter()
            .flatten()
            .filter(|c| **c == Cell::Hit)
            .count();
        hit_count >= total_ship_cells
    }
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Ship placement tests -----------------------------------------------

    #[test]
    fn place_ship_horizontal_success() {
        let mut board = Board::new();
        // Place a Carrier (size 5) at (0,0) horizontally
        assert!(board
            .place_ship(ShipKind::Carrier, 0, 0, Orientation::Horizontal)
            .is_ok());

        // Verify grid cells are marked
        for col in 0..5 {
            assert_eq!(board.grid[0][col], Cell::Ship(ShipKind::Carrier));
        }
        // Cell just past the ship should still be empty
        assert_eq!(board.grid[0][5], Cell::Empty);
    }

    #[test]
    fn place_ship_vertical_success() {
        let mut board = Board::new();
        // Place a Destroyer (size 2) at (3,7) vertically → occupies (3,7) and (3,8)
        assert!(board
            .place_ship(ShipKind::Destroyer, 3, 7, Orientation::Vertical)
            .is_ok());

        assert_eq!(board.grid[7][3], Cell::Ship(ShipKind::Destroyer));
        assert_eq!(board.grid[8][3], Cell::Ship(ShipKind::Destroyer));
    }

    #[test]
    fn place_ship_out_of_bounds_horizontal() {
        let mut board = Board::new();
        // Carrier (size 5) at col 7 → would need cols 7,8,9,10,11 — out of bounds
        let result = board.place_ship(ShipKind::Carrier, 7, 0, Orientation::Horizontal);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Ship placement is out of bounds");
    }

    #[test]
    fn place_ship_out_of_bounds_vertical() {
        let mut board = Board::new();
        // Cruiser (size 3) at row 9 → would need rows 9,10,11 — out of bounds
        let result = board.place_ship(ShipKind::Cruiser, 0, 9, Orientation::Vertical);
        assert!(result.is_err());
    }

    #[test]
    fn place_ship_overlap_rejected() {
        let mut board = Board::new();
        // Place Battleship at (0,0) horizontally → cols 0..4
        board
            .place_ship(ShipKind::Battleship, 0, 0, Orientation::Horizontal)
            .unwrap();

        // Try to place Destroyer at (2,0) vertically → overlaps at (2,0)
        let result = board.place_ship(ShipKind::Destroyer, 2, 0, Orientation::Vertical);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Ship overlaps with an existing ship");
    }

    #[test]
    fn place_all_five_ships() {
        let mut board = Board::new();
        // Place all 5 ships without overlapping
        board
            .place_ship(ShipKind::Carrier, 0, 0, Orientation::Horizontal)
            .unwrap();
        board
            .place_ship(ShipKind::Battleship, 0, 2, Orientation::Horizontal)
            .unwrap();
        board
            .place_ship(ShipKind::Cruiser, 0, 4, Orientation::Horizontal)
            .unwrap();
        board
            .place_ship(ShipKind::Submarine, 0, 6, Orientation::Horizontal)
            .unwrap();
        board
            .place_ship(ShipKind::Destroyer, 0, 8, Orientation::Horizontal)
            .unwrap();

        assert_eq!(board.ships.len(), 5);
    }

    // -- Firing tests -------------------------------------------------------

    #[test]
    fn fire_miss() {
        let mut board = Board::new();
        board
            .place_ship(ShipKind::Destroyer, 0, 0, Orientation::Horizontal)
            .unwrap();

        // Fire at an empty cell
        let (hit, sunk) = board.receive_fire(5, 5);
        assert!(!hit);
        assert!(sunk.is_none());
        assert_eq!(board.grid[5][5], Cell::Miss);
    }

    #[test]
    fn fire_hit_no_sink() {
        let mut board = Board::new();
        board
            .place_ship(ShipKind::Destroyer, 0, 0, Orientation::Horizontal)
            .unwrap();

        // Hit the first cell of the destroyer
        let (hit, sunk) = board.receive_fire(0, 0);
        assert!(hit);
        assert!(sunk.is_none()); // needs 2 hits to sink
        assert_eq!(board.grid[0][0], Cell::Hit);
    }

    #[test]
    fn fire_hit_and_sink() {
        let mut board = Board::new();
        board
            .place_ship(ShipKind::Destroyer, 0, 0, Orientation::Horizontal)
            .unwrap();

        // Hit both cells → should sink
        board.receive_fire(0, 0);
        let (hit, sunk) = board.receive_fire(1, 0);
        assert!(hit);
        assert_eq!(sunk, Some(ShipKind::Destroyer));
    }

    #[test]
    fn fire_at_already_hit_cell() {
        let mut board = Board::new();
        board
            .place_ship(ShipKind::Destroyer, 0, 0, Orientation::Horizontal)
            .unwrap();

        board.receive_fire(0, 0); // first hit
        let (hit, sunk) = board.receive_fire(0, 0); // same cell again
        assert!(!hit); // treated as miss, no double-counting
        assert!(sunk.is_none());
    }

    // -- all_sunk tests -----------------------------------------------------

    #[test]
    fn all_sunk_false_initially() {
        let mut board = Board::new();
        board
            .place_ship(ShipKind::Destroyer, 0, 0, Orientation::Horizontal)
            .unwrap();
        assert!(!board.all_sunk());
    }

    #[test]
    fn all_sunk_true_when_everything_destroyed() {
        let mut board = Board::new();
        // Place two ships
        board
            .place_ship(ShipKind::Destroyer, 0, 0, Orientation::Horizontal)
            .unwrap();
        board
            .place_ship(ShipKind::Submarine, 0, 2, Orientation::Horizontal)
            .unwrap();

        // Sink the Destroyer (size 2)
        board.receive_fire(0, 0);
        board.receive_fire(1, 0);
        assert!(!board.all_sunk()); // Submarine still alive

        // Sink the Submarine (size 3)
        board.receive_fire(0, 2);
        board.receive_fire(1, 2);
        board.receive_fire(2, 2);
        assert!(board.all_sunk());
    }

    #[test]
    fn all_sunk_empty_board_is_vacuously_true() {
        let board = Board::new();
        // No ships → all (zero) ships are sunk
        assert!(board.all_sunk());
    }

    // -- ShipKind helper tests ----------------------------------------------

    #[test]
    fn ship_kind_sizes() {
        assert_eq!(ShipKind::Carrier.size(), 5);
        assert_eq!(ShipKind::Battleship.size(), 4);
        assert_eq!(ShipKind::Cruiser.size(), 3);
        assert_eq!(ShipKind::Submarine.size(), 3);
        assert_eq!(ShipKind::Destroyer.size(), 2);
    }

    #[test]
    fn ship_kind_all_returns_five() {
        assert_eq!(ShipKind::all().len(), 5);
    }

    // -- GameState tests ----------------------------------------------------

    #[test]
    fn game_state_initial() {
        let state = GameState::new("Alice".to_string());
        assert_eq!(state.phase, Phase::Connecting);
        assert_eq!(state.my_name, "Alice");
        assert!(state.opponent_name.is_empty());
        assert!(state.my_board.ships.is_empty());
    }

    #[test]
    fn board_default_matches_new() {
        let default_board: Board = Board::default();
        let new_board = Board::new();
        assert_eq!(default_board.grid, new_board.grid);
        assert!(default_board.ships.is_empty());
    }

    #[test]
    fn ship_kind_display() {
        assert_eq!(format!("{}", ShipKind::Carrier), "Carrier");
        assert_eq!(format!("{}", ShipKind::Destroyer), "Destroyer");
    }

    #[test]
    fn orientation_toggle() {
        assert_eq!(Orientation::Horizontal.toggle(), Orientation::Vertical);
        assert_eq!(Orientation::Vertical.toggle(), Orientation::Horizontal);
    }

    #[test]
    fn has_won_initially_false() {
        let state = GameState::new("Alice".to_string());
        assert!(!state.has_won());
    }

    #[test]
    fn has_won_after_all_hits() {
        let mut state = GameState::new("Alice".to_string());
        // Mark all ship cells as hit on the tracking board
        // Total ship cells = 5 + 4 + 3 + 3 + 2 = 17
        let mut count = 0;
        'outer: for row in 0..GRID_SIZE {
            for col in 0..GRID_SIZE {
                if count >= 17 {
                    break 'outer;
                }
                state.tracking_board[row][col] = Cell::Hit;
                count += 1;
            }
        }
        assert!(state.has_won());
    }

    #[test]
    fn push_chat_keeps_max_entries() {
        let mut state = GameState::new("Alice".to_string());
        // Push more than MAX_CHAT_LOG messages
        for i in 0..8 {
            state.push_chat("Bob".to_string(), format!("msg {}", i));
        }
        // Should only keep the last 5
        assert_eq!(state.chat_log.len(), 5);
        assert_eq!(state.chat_log[0].1, "msg 3");
        assert_eq!(state.chat_log[4].1, "msg 7");
    }

    #[test]
    fn game_state_new_has_empty_chat() {
        let state = GameState::new("Alice".to_string());
        assert!(state.chat_log.is_empty());
    }
}

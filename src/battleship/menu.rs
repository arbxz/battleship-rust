// battleship/menu.rs — Connection setup UI for multiplayer Battleship.
// Handles host/join selection, player name input, and network address entry.

use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Stylize},
    terminal::{self, ClearType},
};

use super::network::{self, Connection};

// ---------------------------------------------------------------------------
// Connection menu — host or join
// ---------------------------------------------------------------------------

/// Prompt the player to choose Host or Join, enter their name, and
/// provide the port (host) or address (join).
/// Returns `(connection, my_name, is_host)` or `Ok(None)` if they pressed Esc.
pub fn connection_menu(stdout: &mut io::Stdout) -> io::Result<Option<(Connection, String, bool)>> {
    execute!(stdout, terminal::Clear(ClearType::All))?;

    // Draw the menu
    execute!(stdout, cursor::MoveTo(4, 2))?;
    write!(stdout, "{}", "╔══════════ BATTLESHIP ══════════╗".with(Color::Cyan))?;
    execute!(stdout, cursor::MoveTo(4, 3))?;
    write!(stdout, "{}", "║                                ║".with(Color::Cyan))?;
    execute!(stdout, cursor::MoveTo(4, 4))?;
    write!(stdout, "{}", "║  1. Host Game                  ║".with(Color::Cyan))?;
    execute!(stdout, cursor::MoveTo(4, 5))?;
    write!(stdout, "{}", "║  2. Join Game                  ║".with(Color::Cyan))?;
    execute!(stdout, cursor::MoveTo(4, 6))?;
    write!(stdout, "{}", "║  Esc. Back to Menu             ║".with(Color::Cyan))?;
    execute!(stdout, cursor::MoveTo(4, 7))?;
    write!(stdout, "{}", "╚════════════════════════════════╝".with(Color::Cyan))?;
    stdout.flush()?;

    // Wait for selection
    let is_host;
    loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('1') => {
                    is_host = true;
                    break;
                }
                KeyCode::Char('2') => {
                    is_host = false;
                    break;
                }
                KeyCode::Esc => return Ok(None),
                _ => {}
            }
        }
    }

    // Get player name
    let name = read_input_line(stdout, 10, "Enter your name: ")?;
    if name.is_empty() {
        return Ok(None);
    }

    // Get port or address
    let conn = if is_host {
        let port_str = read_input_line(stdout, 12, "Port to host on (default 7878): ")?;
        let port: u16 = port_str.trim().parse().unwrap_or(7878);

        // Show waiting message
        execute!(stdout, cursor::MoveTo(4, 14))?;
        write!(
            stdout,
            "{}",
            format!("Hosting on port {} — waiting for opponent...", port).with(Color::Yellow)
        )?;
        stdout.flush()?;

        // Leave raw mode temporarily so the blocking accept doesn't eat input
        terminal::disable_raw_mode()?;
        let conn = network::host(port)?;
        terminal::enable_raw_mode()?;
        conn
    } else {
        let addr = read_input_line(stdout, 12, "Host address (e.g. 127.0.0.1:7878): ")?;
        let addr = if addr.trim().is_empty() {
            "127.0.0.1:7878".to_string()
        } else {
            addr.trim().to_string()
        };

        execute!(stdout, cursor::MoveTo(4, 14))?;
        write!(
            stdout,
            "{}",
            format!("Connecting to {}...", addr).with(Color::Yellow)
        )?;
        stdout.flush()?;

        network::connect(&addr)?
    };

    Ok(Some((conn, name, is_host)))
}

// ---------------------------------------------------------------------------
// Text input helper
// ---------------------------------------------------------------------------

/// Read a line of text input from the user at the given terminal row.
/// Shows `prompt`, then captures typing until Enter is pressed.
/// Returns the entered string. Esc returns an empty string.
pub fn read_input_line(stdout: &mut io::Stdout, row: u16, prompt: &str) -> io::Result<String> {
    execute!(stdout, cursor::MoveTo(4, row))?;
    write!(stdout, "{}", prompt.with(Color::Cyan))?;
    execute!(stdout, cursor::Show)?;
    stdout.flush()?;

    let mut input = String::new();
    loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Enter => {
                    execute!(stdout, cursor::Hide)?;
                    return Ok(input);
                }
                KeyCode::Esc => {
                    execute!(stdout, cursor::Hide)?;
                    return Ok(String::new());
                }
                KeyCode::Backspace => {
                    if !input.is_empty() {
                        input.pop();
                        // Redraw the input area
                        let col = 4 + prompt.len() as u16;
                        execute!(stdout, cursor::MoveTo(col, row))?;
                        write!(stdout, "{}  ", input)?;
                        execute!(
                            stdout,
                            cursor::MoveTo(col + input.len() as u16, row)
                        )?;
                        stdout.flush()?;
                    }
                }
                KeyCode::Char(c) => {
                    if input.len() < 30 {
                        input.push(c);
                        write!(stdout, "{}", c)?;
                        stdout.flush()?;
                    }
                }
                _ => {}
            }
        }
    }
}

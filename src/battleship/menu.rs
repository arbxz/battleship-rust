// battleship/menu.rs — Connection setup UI for multiplayer Battleship.
// Handles host/join selection, player name input, and network address entry.

use std::io::{self, Write};
use std::net::UdpSocket;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Stylize},
    terminal::{self, ClearType},
};

use super::network::{self, Connection};

/// Detect the machine's local LAN IP by briefly opening a UDP socket.
/// Falls back to "127.0.0.1" if detection fails.
fn get_local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

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

    // Get player name (with validation)
    let name = loop {
        let input = read_input_line(stdout, 10, "Enter your name: ")?;
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if trimmed.len() < 2 {
            show_error(stdout, 11, "Name must be at least 2 characters.")?;
            clear_line(stdout, 10)?;
            continue;
        }
        if trimmed.len() > 20 {
            show_error(stdout, 11, "Name must be 20 characters or less.")?;
            clear_line(stdout, 10)?;
            continue;
        }
        clear_line(stdout, 11)?;
        break trimmed;
    };

    // Get port or address
    let conn = if is_host {
        let port: u16 = loop {
            let port_str = read_input_line(stdout, 12, "Port to host on (default 7878): ")?;
            let trimmed = port_str.trim();
            if trimmed.is_empty() {
                break 7878;
            }
            match trimmed.parse::<u16>() {
                Ok(p) if p >= 1024 => {
                    clear_line(stdout, 13)?;
                    break p;
                }
                Ok(_) => {
                    show_error(stdout, 13, "Port must be 1024 or higher.")?;
                    clear_line(stdout, 12)?;
                    continue;
                }
                Err(_) => {
                    show_error(stdout, 13, "Invalid port number.")?;
                    clear_line(stdout, 12)?;
                    continue;
                }
            }
        };

        let local_ip = get_local_ip();

        // Show local IP hints so the other player knows what to connect to
        execute!(stdout, cursor::MoveTo(4, 14))?;
        write!(
            stdout,
            "{}",
            format!("Hosting on 0.0.0.0:{}", port).with(Color::Yellow)
        )?;
        execute!(stdout, cursor::MoveTo(4, 15))?;
        write!(
            stdout,
            "{}",
            format!("Other player should connect to  127.0.0.1:{}  (same PC)", port)
                .with(Color::DarkGrey)
        )?;
        execute!(stdout, cursor::MoveTo(4, 16))?;
        write!(
            stdout,
            "{}",
            format!("  or your LAN IP  {}:{}  (same network)", local_ip, port)
                .with(Color::DarkGrey)
        )?;
        execute!(stdout, cursor::MoveTo(4, 18))?;
        write!(
            stdout,
            "{}",
            "Waiting for opponent...".with(Color::Yellow)
        )?;
        stdout.flush()?;

        // Leave raw mode temporarily so the blocking accept doesn't eat input
        terminal::disable_raw_mode()?;
        let result = network::host(port);
        terminal::enable_raw_mode()?;
        match result {
            Ok(c) => c,
            Err(e) => {
                execute!(stdout, cursor::MoveTo(4, 20))?;
                write!(
                    stdout,
                    "{}",
                    format!("Failed to host: {}  (press any key)", e).with(Color::Red)
                )?;
                stdout.flush()?;
                let _ = event::read();
                return Ok(None);
            }
        }
    } else {
        let addr = loop {
            let input = read_input_line(stdout, 12, "Host address (e.g. 127.0.0.1:7878): ")?;
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                break "127.0.0.1:7878".to_string();
            }
            // If they only typed an IP without a port, add the default port
            let candidate = if !trimmed.contains(':') {
                format!("{}:7878", trimmed)
            } else {
                trimmed
            };
            // Validate the address format
            match candidate.parse::<std::net::SocketAddr>() {
                Ok(_) => {
                    clear_line(stdout, 13)?;
                    break candidate;
                }
                Err(_) => {
                    show_error(stdout, 13, "Invalid address. Use format: IP:PORT (e.g. 192.168.1.5:7878)")?;
                    clear_line(stdout, 12)?;
                    continue;
                }
            }
        };

        execute!(stdout, cursor::MoveTo(4, 14))?;
        write!(
            stdout,
            "{}",
            format!("Connecting to {}...", addr).with(Color::Yellow)
        )?;
        stdout.flush()?;

        match network::connect(&addr) {
            Ok(c) => c,
            Err(e) => {
                execute!(stdout, cursor::MoveTo(4, 16))?;
                write!(
                    stdout,
                    "{}",
                    format!("Failed to connect: {}  (press any key)", e).with(Color::Red)
                )?;
                stdout.flush()?;
                let _ = event::read();
                return Ok(None);
            }
        }
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

// ---------------------------------------------------------------------------
// Small helpers for validation feedback
// ---------------------------------------------------------------------------

/// Show a red error message at the given row.
fn show_error(stdout: &mut io::Stdout, row: u16, msg: &str) -> io::Result<()> {
    execute!(stdout, cursor::MoveTo(4, row))?;
    write!(stdout, "{}", msg.with(Color::Red))?;
    stdout.flush()
}

/// Clear a line so it can be re-drawn.
fn clear_line(stdout: &mut io::Stdout, row: u16) -> io::Result<()> {
    execute!(stdout, cursor::MoveTo(4, row))?;
    write!(stdout, "{}", " ".repeat(70))?;
    stdout.flush()
}

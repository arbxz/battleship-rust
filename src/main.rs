mod battleship;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{self, Color, Stylize},
    terminal::{self, ClearType},
};
use std::io::{self, Write};
use std::time::Duration;

const MENU_W: u16 = 50;

const LOGO: &[&str] = &[
    r"  ____  _   _ ____ _____    __     ___    ____  _____ ____  ",
    r" |  _ \| | | / ___|_   _|   \ \   / / \  |  _ \| ____|  _ \ ",
    r" | |_) | | | \___ \ | |      \ \ / / _ \ | | | |  _| | |_) |",
    r" |  _ <| |_| |___) || |       \ V / ___ \| |_| | |___|  _ < ",
    r" |_| \_\\___/|____/ |_|        \_/_/   \_\____/|_____|_| \_\\",
];

struct MenuItem {
    label: &'static str,
    desc: &'static str,
}

const MENU_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Battleship",
        desc: "Multiplayer naval warfare over network!",
    },
    MenuItem {
        label: "Quit",
        desc: "Exit the game",
    },
];

fn draw_menu(stdout: &mut io::Stdout, selected: usize) -> io::Result<()> {
    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All),
    )?;

    let (term_w, term_h) = terminal::size().unwrap_or((80, 30));
    let start_y = 2u16;

    // Logo
    for (i, line) in LOGO.iter().enumerate() {
        let lx = term_w.saturating_sub(line.len() as u16) / 2;
        execute!(
            stdout,
            cursor::MoveTo(lx, start_y + i as u16),
            style::SetForegroundColor(Color::Cyan),
        )?;
        write!(stdout, "{}", line)?;
    }

    // Subtitle
    let subtitle = "── Terminal Arcade ──";
    let sx = term_w.saturating_sub(subtitle.len() as u16) / 2;
    execute!(
        stdout,
        cursor::MoveTo(sx, start_y + LOGO.len() as u16 + 1),
        style::SetForegroundColor(Color::DarkGrey),
    )?;
    write!(stdout, "{}", subtitle)?;

    // Menu items
    let menu_y = start_y + LOGO.len() as u16 + 4;
    let box_x = term_w.saturating_sub(MENU_W) / 2;

    // Top border
    execute!(
        stdout,
        cursor::MoveTo(box_x, menu_y),
        style::SetForegroundColor(Color::DarkGrey),
    )?;
    write!(stdout, "┌{}┐", "─".repeat(MENU_W as usize - 2))?;

    for (i, item) in MENU_ITEMS.iter().enumerate() {
        let y = menu_y + 1 + (i as u16 * 3);

        // Item line
        execute!(stdout, cursor::MoveTo(box_x, y), style::SetForegroundColor(Color::DarkGrey))?;
        write!(stdout, "│")?;

        let prefix = if i == selected { " ▶ " } else { "   " };
        let label_str = format!("{}{}", prefix, item.label);
        let pad = (MENU_W as usize - 2).saturating_sub(label_str.len());

        if i == selected {
            execute!(
                stdout,
                style::PrintStyledContent(
                    format!("{}{}", label_str, " ".repeat(pad))
                        .on(Color::DarkBlue)
                        .with(Color::White)
                ),
            )?;
        } else {
            execute!(stdout, style::SetForegroundColor(Color::White))?;
            write!(stdout, "{}{}", label_str, " ".repeat(pad))?;
        }
        execute!(stdout, style::SetForegroundColor(Color::DarkGrey))?;
        write!(stdout, "│")?;

        // Description line
        execute!(stdout, cursor::MoveTo(box_x, y + 1), style::SetForegroundColor(Color::DarkGrey))?;
        write!(stdout, "│")?;
        let desc_str = format!("     {}", item.desc);
        let dpad = (MENU_W as usize - 2).saturating_sub(desc_str.len());
        execute!(stdout, style::SetForegroundColor(Color::DarkGrey))?;
        write!(stdout, "{}{}", desc_str, " ".repeat(dpad))?;
        execute!(stdout, style::SetForegroundColor(Color::DarkGrey))?;
        write!(stdout, "│")?;

        // Separator
        if i < MENU_ITEMS.len() - 1 {
            execute!(stdout, cursor::MoveTo(box_x, y + 2), style::SetForegroundColor(Color::DarkGrey))?;
            write!(stdout, "├{}┤", "─".repeat(MENU_W as usize - 2))?;
        }
    }

    // Bottom border
    let bot_y = menu_y + 1 + (MENU_ITEMS.len() as u16 * 3) - 1;
    execute!(stdout, cursor::MoveTo(box_x, bot_y), style::SetForegroundColor(Color::DarkGrey))?;
    write!(stdout, "└{}┘", "─".repeat(MENU_W as usize - 2))?;

    // Controls
    let controls = "↑↓ Navigate  |  ENTER Select  |  Q Quit";
    let cx = term_w.saturating_sub(controls.len() as u16) / 2;
    execute!(
        stdout,
        cursor::MoveTo(cx, bot_y + 2),
        style::SetForegroundColor(Color::DarkGrey),
    )?;
    write!(stdout, "{}", controls)?;

    // Footer
    let footer = "Made with Rust + Crossterm";
    let fx = term_w.saturating_sub(footer.len() as u16) / 2;
    let fy = term_h.saturating_sub(2);
    execute!(
        stdout,
        cursor::MoveTo(fx, fy),
        style::SetForegroundColor(Color::DarkGrey),
    )?;
    write!(stdout, "{}", footer)?;

    execute!(stdout, style::ResetColor)?;
    stdout.flush()
}

fn run_menu(stdout: &mut io::Stdout) -> io::Result<()> {
    let mut selected: usize = 0;

    loop {
        execute!(stdout, terminal::Clear(ClearType::All))?;
        draw_menu(stdout, selected)?;

        // Block until key event
        loop {
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => {
                            if selected > 0 {
                                selected -= 1;
                            } else {
                                selected = MENU_ITEMS.len() - 1;
                            }
                            break;
                        }
                        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => {
                            selected = (selected + 1) % MENU_ITEMS.len();
                            break;
                        }
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            match selected {
                                0 => {
                                    execute!(stdout, terminal::Clear(ClearType::All))?;
                                    battleship::run()?;
                                }
                                1 => return Ok(()),
                                _ => {}
                            }
                            break;
                        }
                        KeyCode::Char('1') => {
                            execute!(stdout, terminal::Clear(ClearType::All))?;
                            battleship::run()?;
                            break;
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        terminal::Clear(ClearType::All),
    )?;

    let result = run_menu(&mut stdout);

    execute!(
        stdout,
        cursor::Show,
        terminal::LeaveAlternateScreen,
    )?;
    terminal::disable_raw_mode()?;

    result
}

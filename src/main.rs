mod cert;
mod fzf;
mod ui;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ui::{ACTIVE, DANGER, DIM, TEXT, WARN};

#[derive(Clone)]
enum State {
    Main,
    Select,
    RemoveSelect,
    RemoveConfirm { cert_idx: usize, yes_selected: bool },
    Toast { tag: String, msg: String, danger: bool },
}

fn main() -> Result<()> {
    if !fzf::is_installed() {
        // Only block on this once, up front, in normal (cooked) mode.
        fzf::prompt_install()?;
    }

    enable_raw_mode()?;
    let result = run();
    disable_raw_mode()?;
    ui::show_cursor();
    println!();
    result
}

/// Runs a closure with raw mode temporarily disabled, so any `sudo` prompt
/// it triggers (password entry, echo handling) behaves normally. Restores
/// raw mode afterwards regardless of the closure's outcome.
fn with_cooked_terminal<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    disable_raw_mode()?;
    ui::show_cursor();
    println!();
    let result = f();
    enable_raw_mode()?;
    result
}

fn run() -> Result<()> {
    let mut state = State::Main;
    let mut cursor: usize = 0;

    loop {
        let certs = cert::list()?;
        ui::clear_screen();

        match &state {
            State::Main => {
                ui::print_prompt("certificate-manager");
                let lines: Vec<(String, ui::Color)> = certs
                    .iter()
                    .map(|c| (ui::cert_line(c), if c.active { ACTIVE } else { DIM }))
                    .collect();
                ui::draw_panel("CERTIFICATES", &lines);

                let options = ["ADD", "SELECT", "REMOVE", "QUIT"];
                let opt_lines: Vec<(String, ui::Color)> = options
                    .iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let text = format!("[{o}]");
                        let color = if i == cursor { ACTIVE } else { TEXT };
                        (text, color)
                    })
                    .collect();
                ui::draw_panel("OPTIONS", &opt_lines);
                ui::print_hint("UP/DOWN navigate    ENTER select    Q quit");
                ui::flush();

                match read_key()? {
                    KeyAction::Up => cursor = (cursor + 3) % 4,
                    KeyAction::Down => cursor = (cursor + 1) % 4,
                    KeyAction::Char('q') | KeyAction::Char('Q') => break,
                    KeyAction::Enter => match cursor {
                        0 => state = do_add()?,
                        1 => {
                            state = State::Select;
                            cursor = 0;
                        }
                        2 => {
                            state = State::RemoveSelect;
                            cursor = 0;
                        }
                        3 => break,
                        _ => {}
                    },
                    KeyAction::Esc => break,
                    _ => {}
                }
            }

            State::Select => {
                ui::print_prompt("certificate-manager select");
                if certs.is_empty() {
                    ui::draw_panel("SELECT A CERTIFICATE", &[]);
                } else {
                    let lines: Vec<(String, ui::Color)> = certs
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            let color = if i == cursor { ACTIVE } else if c.active { ACTIVE } else { DIM };
                            (ui::cert_line(c), color)
                        })
                        .collect();
                    ui::draw_panel("SELECT A CERTIFICATE", &lines);
                }
                ui::print_hint("UP/DOWN navigate    ENTER toggle on/off    ESC back");
                ui::flush();

                match read_key()? {
                    KeyAction::Up if !certs.is_empty() => {
                        cursor = (cursor + certs.len() - 1) % certs.len()
                    }
                    KeyAction::Down if !certs.is_empty() => cursor = (cursor + 1) % certs.len(),
                    KeyAction::Enter if !certs.is_empty() => {
                        let target = &certs[cursor];
                        if target.active {
                            match with_cooked_terminal(|| cert::deactivate(target.id)) {
                                Ok(()) => {
                                    state = State::Toast {
                                        tag: "OK".into(),
                                        msg: format!("{} deactivated", ui::cert_line(target)),
                                        danger: false,
                                    }
                                }
                                Err(e) => {
                                    state = State::Toast {
                                        tag: "ERROR".into(),
                                        msg: e.to_string(),
                                        danger: true,
                                    }
                                }
                            }
                        } else {
                            match with_cooked_terminal(|| cert::activate(target.id)) {
                                Ok(()) => {
                                    state = State::Toast {
                                        tag: "OK".into(),
                                        msg: format!("{} activated", ui::cert_line(target)),
                                        danger: false,
                                    }
                                }
                                Err(e) => {
                                    state = State::Toast {
                                        tag: "ERROR".into(),
                                        msg: e.to_string(),
                                        danger: true,
                                    }
                                }
                            }
                        }
                        cursor = 0;
                    }
                    KeyAction::Esc => {
                        state = State::Main;
                        cursor = 0;
                    }
                    _ => {}
                }
            }

            State::RemoveSelect => {
                ui::print_prompt("certificate-manager remove");
                if certs.is_empty() {
                    ui::draw_panel("REMOVE A CERTIFICATE", &[]);
                } else {
                    let lines: Vec<(String, ui::Color)> = certs
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            let color = if i == cursor { ACTIVE } else if c.active { ACTIVE } else { DIM };
                            (ui::cert_line(c), color)
                        })
                        .collect();
                    ui::draw_panel("REMOVE A CERTIFICATE", &lines);
                }
                ui::print_hint("UP/DOWN navigate    ENTER select    ESC back");
                ui::flush();

                match read_key()? {
                    KeyAction::Up if !certs.is_empty() => {
                        cursor = (cursor + certs.len() - 1) % certs.len()
                    }
                    KeyAction::Down if !certs.is_empty() => cursor = (cursor + 1) % certs.len(),
                    KeyAction::Enter if !certs.is_empty() => {
                        state = State::RemoveConfirm {
                            cert_idx: cursor,
                            yes_selected: true,
                        };
                    }
                    KeyAction::Esc => {
                        state = State::Main;
                        cursor = 0;
                    }
                    _ => {}
                }
            }

            State::RemoveConfirm { cert_idx, yes_selected } => {
                let cert_idx = *cert_idx;
                let yes_selected = *yes_selected;
                ui::print_prompt("certificate-manager remove");
                let lines: Vec<(String, ui::Color)> = certs
                    .iter()
                    .map(|c| (ui::cert_line(c), if c.active { ACTIVE } else { DIM }))
                    .collect();
                ui::draw_panel("REMOVE A CERTIFICATE", &lines);

                let target = &certs[cert_idx];
                let warn_line = format!(
                    "{} will be permanently removed. This cannot be undone.",
                    ui::cert_line(target)
                );
                let options_line = if yes_selected {
                    "> [YES, REMOVE]    [NO, CANCEL]"
                } else {
                    "[YES, REMOVE]    > [NO, CANCEL]"
                };
                ui::draw_panel(
                    "CONFIRM REMOVAL",
                    &[(warn_line, WARN), (options_line.to_string(), DANGER)],
                );

                ui::print_hint("LEFT/RIGHT choose    ENTER confirm    ESC cancel");
                ui::flush();

                match read_key()? {
                    KeyAction::Left | KeyAction::Right => {
                        state = State::RemoveConfirm { cert_idx, yes_selected: !yes_selected };
                    }
                    KeyAction::Enter => {
                        if yes_selected {
                            match with_cooked_terminal(|| cert::remove(target.id)) {
                                Ok(removed) => {
                                    state = State::Toast {
                                        tag: "REMOVED".into(),
                                        msg: format!(
                                            "{} permanently removed",
                                            ui::cert_line(&removed)
                                        ),
                                        danger: true,
                                    }
                                }
                                Err(e) => {
                                    state = State::Toast {
                                        tag: "ERROR".into(),
                                        msg: e.to_string(),
                                        danger: true,
                                    }
                                }
                            }
                        } else {
                            state = State::Toast {
                                tag: "CANCELLED".into(),
                                msg: "no changes made".into(),
                                danger: false,
                            };
                        }
                        cursor = 0;
                    }
                    KeyAction::Esc => {
                        state = State::Toast {
                            tag: "CANCELLED".into(),
                            msg: "no changes made".into(),
                            danger: false,
                        };
                        cursor = 0;
                    }
                    _ => {}
                }
            }

            State::Toast { tag, msg, danger } => {
                ui::print_prompt("certificate-manager");
                let lines: Vec<(String, ui::Color)> = certs
                    .iter()
                    .map(|c| (ui::cert_line(c), if c.active { ACTIVE } else { DIM }))
                    .collect();
                ui::draw_panel("CERTIFICATES", &lines);
                ui::print_toast(tag, msg, *danger);
                ui::print_hint("ENTER continue");
                ui::flush();

                match read_key()? {
                    KeyAction::Enter | KeyAction::Esc => {
                        state = State::Main;
                        cursor = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

/// Runs the "add certificate" flow: temporarily drops out of raw mode so
/// fzf (and sudo, if it needs to install itself) can take over the TTY.
fn do_add() -> Result<State> {
    disable_raw_mode()?;
    ui::show_cursor();
    println!();

    let picked = fzf::pick_file();

    enable_raw_mode()?;
    let picked = picked?;

    Ok(match picked {
        Some(path) => match cert::add(std::path::Path::new(&path)) {
            Ok(c) => State::Toast {
                tag: "ADDED".into(),
                msg: format!("{} added", ui::cert_line(&c)),
                danger: false,
            },
            Err(e) => State::Toast {
                tag: "ERROR".into(),
                msg: e.to_string(),
                danger: true,
            },
        },
        None => State::Toast {
            tag: "CANCELLED".into(),
            msg: "no file selected".into(),
            danger: false,
        },
    })
}

enum KeyAction {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Char(char),
    Other,
}

fn read_key() -> Result<KeyAction> {
    loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            return Ok(match key.code {
                KeyCode::Up => KeyAction::Up,
                KeyCode::Down => KeyAction::Down,
                KeyCode::Left => KeyAction::Left,
                KeyCode::Right => KeyAction::Right,
                KeyCode::Enter => KeyAction::Enter,
                KeyCode::Esc => KeyAction::Esc,
                KeyCode::Char(c) => KeyAction::Char(c),
                _ => KeyAction::Other,
            });
        }
    }
}

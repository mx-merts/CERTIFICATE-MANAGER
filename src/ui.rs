use crossterm::cursor::{Hide, MoveTo, Show};
pub use crossterm::style::Color;
use crossterm::style::{Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{stdout, Stdout, Write};

use crate::cert::Certificate;

pub const ACTIVE: Color = Color::Rgb { r: 79, g: 214, b: 196 };
pub const WARN: Color = Color::Rgb { r: 224, g: 166, b: 64 };
pub const DANGER: Color = Color::Rgb { r: 224, g: 96, b: 90 };
pub const DIM: Color = Color::Rgb { r: 95, g: 114, b: 116 };
pub const TEXT: Color = Color::Rgb { r: 215, g: 224, b: 224 };
const PROMPT: Color = Color::Rgb { r: 127, g: 168, b: 160 };

// IMPORTANT: in raw mode the terminal does NOT translate "\n" into a
// carriage return + line feed, so every explicit newline in this module
// must be "\r\n" or lines will stair-step to the right. All drawing goes
// through this one writer so nothing gets interleaved out of order.

pub fn clear_screen() {
    let mut out = stdout();
    execute!(out, Clear(ClearType::All), MoveTo(0, 0), Hide).ok();
}

pub fn show_cursor() {
    execute!(stdout(), Show).ok();
}

fn colored(out: &mut Stdout, text: &str, color: Color) {
    queue!(out, SetForegroundColor(color), Print(text), ResetColor).ok();
}

fn newline(out: &mut Stdout) {
    queue!(out, Print("\r\n")).ok();
}

/// Formats one certificate row exactly as: [ACTIVE]-[12]-[my_cert.cer]
pub fn cert_line(c: &Certificate) -> String {
    let status = if c.active { "ACTIVE" } else { "INACTIVE" };
    format!("[{status}]-[{}]-[{}]", c.id, c.name)
}

/// Draws a boxed panel with a label cut into the top border, sized to fit
/// its longest content line. Every row (top border, content, bottom
/// border) is exactly `w + 4` characters wide.
pub fn draw_panel(label: &str, lines: &[(String, Color)]) {
    let mut out = stdout();

    let content_max = lines
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0)
        .max("no certificates yet".chars().count());
    let w = content_max.max(label.chars().count() + 2);

    newline(&mut out);

    let top_prefix = format!("┌─ {label} ");
    let top_fill_len = w.saturating_sub(label.chars().count() + 1).max(1);
    colored(&mut out, &top_prefix, DIM);
    colored(&mut out, &format!("{}┐", "─".repeat(top_fill_len)), DIM);
    newline(&mut out);

    if lines.is_empty() {
        colored(&mut out, "│ ", DIM);
        colored(&mut out, "no certificates yet", DIM);
        let pad = w.saturating_sub("no certificates yet".chars().count());
        colored(&mut out, &" ".repeat(pad), TEXT);
        colored(&mut out, " │", DIM);
        newline(&mut out);
    }

    for (line, color) in lines {
        colored(&mut out, "│ ", DIM);
        colored(&mut out, line, *color);
        let pad = w.saturating_sub(line.chars().count());
        colored(&mut out, &" ".repeat(pad), TEXT);
        colored(&mut out, " │", DIM);
        newline(&mut out);
    }

    colored(&mut out, &format!("└{}┘", "─".repeat(w + 2)), DIM);
    newline(&mut out);
}

pub fn print_prompt(cmd: &str) {
    let mut out = stdout();
    colored(&mut out, "$ ", PROMPT);
    colored(&mut out, cmd, TEXT);
    newline(&mut out);
}

pub fn print_hint(text: &str) {
    let mut out = stdout();
    newline(&mut out);
    colored(&mut out, text, DIM);
    newline(&mut out);
}

pub fn print_toast(tag: &str, rest: &str, is_danger: bool) {
    let mut out = stdout();
    newline(&mut out);
    colored(&mut out, &format!("[{tag}]"), if is_danger { DANGER } else { ACTIVE });
    colored(&mut out, &format!(" {rest}"), TEXT);
    newline(&mut out);
}

pub fn flush() {
    stdout().flush().ok();
}

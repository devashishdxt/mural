use super::{Color, Line, Span, Style, TextError};
use ansi_str::{AnsiStr, Color as AnsiColor, Style as AnsiStyle, get_blocks};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseMode {
    Raw,
    Ansi,
}

pub(crate) fn parse_text(content: &str, mode: ParseMode) -> Result<Vec<Line>, TextError> {
    let normalized = normalize_newlines(content);

    normalized
        .ansi_split("\n")
        .map(|line| match mode {
            ParseMode::Raw => raw_line(line.as_ref()),
            ParseMode::Ansi => ansi_line(line.as_ref()),
        })
        .collect()
}

fn raw_line(content: &str) -> Result<Line, TextError> {
    let plain = sanitize_text(&content.ansi_strip());
    Line::from_plain(plain)
}

fn ansi_line(content: &str) -> Result<Line, TextError> {
    let mut builder = SpanBuilder::new();

    for block in get_blocks(content) {
        builder.push(
            sanitize_text(&block.text().ansi_strip()),
            brisk_style(block.style()),
        )?;
    }

    Ok(Line::from_spans(builder.finish()?))
}

struct SpanBuilder {
    spans: Vec<Span>,
    content: String,
    style: Style,
}

impl SpanBuilder {
    fn new() -> Self {
        Self {
            spans: Vec::new(),
            content: String::new(),
            style: Style::new(),
        }
    }

    fn push(&mut self, content: String, style: Style) -> Result<(), TextError> {
        if content.is_empty() {
            return Ok(());
        }

        if !self.content.is_empty() && self.style != style {
            self.flush()?;
        }

        self.style = style;
        self.content.push_str(&content);
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<Span>, TextError> {
        self.flush()?;
        Ok(self.spans)
    }

    fn flush(&mut self) -> Result<(), TextError> {
        if self.content.is_empty() {
            return Ok(());
        }

        self.spans
            .push(Span::new(std::mem::take(&mut self.content), self.style)?);
        Ok(())
    }
}

fn brisk_style(style: &AnsiStyle) -> Style {
    let mut brisk = Style::new();

    if let Some(color) = style.foreground() {
        brisk = brisk.fg(brisk_color(color));
    }
    if let Some(color) = style.background() {
        brisk = brisk.bg(brisk_color(color));
    }
    if style.is_bold() {
        brisk = brisk.bold();
    }
    if style.is_faint() {
        brisk = brisk.dim();
    }
    if style.is_italic() {
        brisk = brisk.italic();
    }
    if style.is_underline() {
        brisk = brisk.underline();
    }
    if style.is_inverse() {
        brisk = brisk.reversed();
    }

    brisk
}

fn brisk_color(color: AnsiColor) -> Color {
    match color {
        AnsiColor::Black => Color::Black,
        AnsiColor::Red => Color::Red,
        AnsiColor::Green => Color::Green,
        AnsiColor::Yellow => Color::Yellow,
        AnsiColor::Blue => Color::Blue,
        AnsiColor::Magenta | AnsiColor::Purple => Color::Magenta,
        AnsiColor::Cyan => Color::Cyan,
        AnsiColor::White => Color::White,
        AnsiColor::BrightBlack => Color::BrightBlack,
        AnsiColor::BrightRed => Color::BrightRed,
        AnsiColor::BrightGreen => Color::BrightGreen,
        AnsiColor::BrightYellow => Color::BrightYellow,
        AnsiColor::BrightBlue => Color::BrightBlue,
        AnsiColor::BrightMagenta | AnsiColor::BrightPurple => Color::BrightMagenta,
        AnsiColor::BrightCyan => Color::BrightCyan,
        AnsiColor::BrightWhite => Color::BrightWhite,
        AnsiColor::Fixed(index) => Color::Indexed(index),
        AnsiColor::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
    }
}

fn normalize_newlines(content: &str) -> String {
    let mut normalized = String::new();
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push('\n');
            }
            '\n' => {
                if chars.peek() == Some(&'\r') {
                    chars.next();
                }
                normalized.push('\n');
            }
            _ => normalized.push(ch),
        }
    }

    normalized
}

fn sanitize_text(content: &str) -> String {
    let mut sanitized = String::new();
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\t' => sanitized.push_str("    "),
            '\u{009b}' => skip_c1_control_sequence(&mut chars),
            '\u{0090}' | '\u{0098}' | '\u{009d}' | '\u{009e}' | '\u{009f}' => {
                skip_c1_string_control(&mut chars)
            }
            _ if is_stripped_control(ch) => {}
            _ => sanitized.push(ch),
        }
    }

    sanitized
}

fn skip_c1_control_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    for ch in chars.by_ref() {
        if ('@'..='~').contains(&ch) {
            break;
        }
    }
}

fn skip_c1_string_control(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(ch) = chars.next() {
        if ch == '\x07' || ch == '\u{009c}' {
            return;
        }
        if ch == '\x1b' && chars.peek() == Some(&'\\') {
            chars.next();
            return;
        }
    }
}

fn is_stripped_control(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'
        )
}

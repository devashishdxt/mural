use std::{borrow::Cow, iter::Peekable, str::Chars};

use ansi_str::{AnsiStr, Color as AnsiColor, Style as AnsiStyle, get_blocks};

use super::{Color, Line, Span, Style, TextError};

const TAB_REPLACEMENT: &str = "    ";
type CharStream<'a> = Peekable<Chars<'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseMode {
    Raw,
    Ansi,
}

pub(crate) fn parse_text(content: &str, mode: ParseMode) -> Result<Vec<Line>, TextError> {
    let normalized = normalize_newlines(content);

    normalized
        .as_ref()
        .ansi_split("\n")
        .map(|line| match mode {
            ParseMode::Raw => raw_line(line.as_ref()),
            ParseMode::Ansi => ansi_line(line.as_ref()),
        })
        .collect()
}

fn raw_line(content: &str) -> Result<Line, TextError> {
    let stripped = content.ansi_strip();
    Line::from_plain(sanitize_text(stripped.as_ref()).into_owned())
}

fn ansi_line(content: &str) -> Result<Line, TextError> {
    let mut builder = SpanBuilder::default();

    for block in get_blocks(content) {
        let stripped = block.text().ansi_strip();
        builder.push(
            sanitize_text(stripped.as_ref()),
            style_from_ansi(block.style()),
        );
    }

    Ok(Line::from_spans(builder.finish()))
}

#[derive(Default)]
struct SpanBuilder {
    spans: Vec<Span>,
    content: String,
    style: Style,
}

impl SpanBuilder {
    fn push(&mut self, content: impl AsRef<str>, style: Style) {
        let content = content.as_ref();
        if content.is_empty() {
            return;
        }

        if !self.content.is_empty() && self.style != style {
            self.flush();
        }

        self.style = style;
        self.content.push_str(content);
    }

    fn finish(mut self) -> Vec<Span> {
        self.flush();
        self.spans
    }

    fn flush(&mut self) {
        if self.content.is_empty() {
            return;
        }

        self.spans.push(Span::from_trusted_content(
            std::mem::take(&mut self.content),
            self.style,
        ));
    }
}

fn style_from_ansi(style: &AnsiStyle) -> Style {
    let mut converted = Style::new();

    if let Some(color) = style.foreground() {
        converted = converted.fg(color_from_ansi(color));
    }
    if let Some(color) = style.background() {
        converted = converted.bg(color_from_ansi(color));
    }
    if style.is_bold() {
        converted = converted.bold();
    }
    if style.is_faint() {
        converted = converted.dim();
    }
    if style.is_italic() {
        converted = converted.italic();
    }
    if style.is_underline() {
        converted = converted.underline();
    }
    if style.is_inverse() {
        converted = converted.reversed();
    }

    converted
}

fn color_from_ansi(color: AnsiColor) -> Color {
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

fn normalize_newlines(content: &str) -> Cow<'_, str> {
    if !content.contains('\r') {
        return Cow::Borrowed(content);
    }

    let mut normalized = String::with_capacity(content.len());
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

    Cow::Owned(normalized)
}

fn sanitize_text(content: &str) -> Cow<'_, str> {
    if !content.chars().any(needs_sanitizing) {
        return Cow::Borrowed(content);
    }

    let mut sanitized = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\t' => sanitized.push_str(TAB_REPLACEMENT),
            '\u{009b}' => skip_c1_control_sequence(&mut chars),
            '\u{0090}' | '\u{0098}' | '\u{009d}' | '\u{009e}' | '\u{009f}' => {
                skip_c1_string_control(&mut chars)
            }
            _ if is_stripped_control(ch) => {}
            _ => sanitized.push(ch),
        }
    }

    Cow::Owned(sanitized)
}

fn needs_sanitizing(ch: char) -> bool {
    ch == '\t'
        || ch == '\u{009b}'
        || matches!(
            ch,
            '\u{0090}' | '\u{0098}' | '\u{009d}' | '\u{009e}' | '\u{009f}'
        )
        || is_stripped_control(ch)
}

fn skip_c1_control_sequence(chars: &mut CharStream<'_>) {
    for ch in chars.by_ref() {
        if ('@'..='~').contains(&ch) {
            break;
        }
    }
}

fn skip_c1_string_control(chars: &mut CharStream<'_>) {
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

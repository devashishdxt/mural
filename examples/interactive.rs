use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode as CrosstermKeyCode,
        KeyEvent as CrosstermKeyEvent, KeyEventKind, KeyModifiers as CrosstermKeyModifiers,
    },
    execute, terminal as crossterm_terminal,
};
use mural::{
    Color, Hr, KeyOutcome, Line, ListItem, Padding, Render, Size, Span, StdoutBackend, Style,
    Terminal, Text, TextError, Textarea,
};

const FPS: u64 = 30;
const FRAME_DELAY: Duration = Duration::from_millis(1_000 / FPS);
const ID_INPUT: &str = "input";
const ID_PREVIEW: &str = "preview";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run this example manually in a real terminal:
    //
    //     cargo run --example interactive
    //
    // It uses crossterm for raw-mode keyboard events while Mural owns only the
    // normal-buffer rendering surface.
    let mut session = InteractiveSession::start()?;
    let mut size = current_size()?;

    setup_ui(session.terminal_mut())?;
    run_event_loop(&mut session, &mut size)?;
    session.finish()?;

    println!("finished: submitted messages remain above.");
    Ok(())
}

fn setup_ui(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal.insert_pinned(ID_PREVIEW, Preview::new())?;
    terminal.push_pinned(separator())?;
    terminal.insert_pinned(
        ID_INPUT,
        Textarea::new()
            .placeholder("type something…")?
            .placeholder_style(Style::new().fg(Color::BrightBlack).dim())
            .max_height(5),
    )?;
    terminal.push_pinned(separator())?;
    terminal.push_pinned(help_text()?)?;
    terminal.render()?;
    Ok(())
}

fn run_event_loop(
    session: &mut InteractiveSession,
    size: &mut Size,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut dirty = false;
    let mut last_render = Instant::now();

    loop {
        if dirty && last_render.elapsed() >= FRAME_DELAY {
            session.terminal_mut().render()?;
            last_render = Instant::now();
            dirty = false;
        }

        let poll_timeout = if dirty {
            FRAME_DELAY.saturating_sub(last_render.elapsed())
        } else {
            FRAME_DELAY
        };

        if !event::poll(poll_timeout)? {
            continue;
        }

        match event::read()? {
            Event::Key(key) => {
                if should_quit(key) {
                    break;
                }
                dirty |= handle_input_key(session.terminal_mut(), key, *size)?;
            }
            Event::Resize(width, height) => {
                *size = Size::new(width, height);
                session.terminal_mut().resize(*size)?;
                dirty = true;
            }
            Event::Paste(text) => {
                handle_paste(session.terminal_mut(), text)?;
                dirty = true;
            }
            Event::FocusGained | Event::FocusLost | Event::Mouse(_) => {}
        }
    }

    if dirty {
        session.terminal_mut().render()?;
    }

    Ok(())
}

fn handle_input_key(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
    key: CrosstermKeyEvent,
    size: Size,
) -> Result<bool, Box<dyn std::error::Error>> {
    let outcome = terminal
        .pinned_block_mut::<Textarea>(ID_INPUT)?
        .handle_key_event(key, size.width());

    match outcome {
        KeyOutcome::Submit => submit_input(terminal)?,
        KeyOutcome::Changed | KeyOutcome::Unchanged => update_preview(terminal)?,
        KeyOutcome::Ignored => {}
    }

    Ok(!matches!(outcome, KeyOutcome::Ignored))
}

fn handle_paste(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
    text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal
        .pinned_block_mut::<Textarea>(ID_INPUT)?
        .insert_str(text);
    update_preview(terminal)?;
    Ok(())
}

fn submit_input(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let submitted = terminal.pinned_block_mut::<Textarea>(ID_INPUT)?.take();
    if submitted.is_empty() {
        update_preview(terminal)?;
        return Ok(());
    }

    terminal.push_live(submitted_message(&submitted)?)?;
    update_preview(terminal)?;
    Ok(())
}

fn update_preview(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let value = terminal
        .pinned_block_mut::<Textarea>(ID_INPUT)?
        .value()
        .to_owned();
    terminal
        .pinned_block_mut::<Preview>(ID_PREVIEW)?
        .set_value(value);
    Ok(())
}

fn should_quit(key: CrosstermKeyEvent) -> bool {
    if key.kind == KeyEventKind::Release {
        return false;
    }

    matches!(key.code, CrosstermKeyCode::Esc)
        || matches!(key.code, CrosstermKeyCode::Char('c' | 'C') if key.modifiers.contains(CrosstermKeyModifiers::CONTROL))
}

fn submitted_message(content: &str) -> Result<Padding<ListItem<Text>>, TextError> {
    Ok(Padding::new(
        ListItem::new(input_text(content, Style::new())?)
            .bullet("›")?
            .bullet_style(Style::new().fg(Color::BrightCyan).bold())
            .gap(1),
    )
    .top(1)
    .left(1))
}

fn separator() -> Hr {
    Hr::new().style(Style::new().fg(Color::BrightBlack))
}

fn help_text() -> Result<Text, TextError> {
    styled_text(
        "Enter submit · Shift/Alt+Enter newline · Esc/Ctrl+C quit",
        Style::new().fg(Color::BrightWhite),
    )
}

fn styled_text(content: &str, style: Style) -> Result<Text, TextError> {
    Ok(Text::from_lines(vec![Line::from_spans(vec![Span::new(
        content, style,
    )?])]))
}

fn input_text(content: &str, style: Style) -> Result<Text, TextError> {
    content
        .split('\n')
        .map(|line| {
            let line = line.replace('\t', "    ");
            if line.is_empty() {
                Ok(Line::from_spans(Vec::new()))
            } else {
                Ok(Line::from_spans(vec![Span::new(line, style)?]))
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Text::from_lines)
}

fn current_size() -> io::Result<Size> {
    let (width, height) = crossterm_terminal::size()?;
    Ok(Size::new(width, height))
}

struct Preview {
    value: String,
}

impl Preview {
    fn new() -> Self {
        Self {
            value: String::new(),
        }
    }

    fn set_value(&mut self, value: String) -> &mut Self {
        self.value = value;
        self
    }
}

impl Render for Preview {
    fn render(&self, width: u16) -> Text {
        if self.value.is_empty() {
            return Text::empty();
        }

        ListItem::new(
            input_text(&self.value, Style::new()).expect("textarea content is sanitized text"),
        )
        .bullet("~")
        .expect("preview bullet is static valid text")
        .bullet_style(Style::new().fg(Color::BrightYellow).dim())
        .gap(1)
        .render(width)
    }
}

struct InteractiveSession {
    terminal: Terminal<StdoutBackend<io::Stdout>>,
    raw_mode: bool,
    bracketed_paste: bool,
}

impl InteractiveSession {
    fn start() -> io::Result<Self> {
        crossterm_terminal::enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnableBracketedPaste) {
            let _ = crossterm_terminal::disable_raw_mode();
            return Err(error);
        }

        match Terminal::stdout() {
            Ok(terminal) => Ok(Self {
                terminal,
                raw_mode: true,
                bracketed_paste: true,
            }),
            Err(error) => {
                let _ = execute!(io::stdout(), DisableBracketedPaste);
                let _ = crossterm_terminal::disable_raw_mode();
                Err(error)
            }
        }
    }

    fn terminal_mut(&mut self) -> &mut Terminal<StdoutBackend<io::Stdout>> {
        &mut self.terminal
    }

    fn finish(&mut self) -> io::Result<()> {
        self.terminal.finish()?;
        if self.bracketed_paste {
            execute!(io::stdout(), DisableBracketedPaste)?;
            self.bracketed_paste = false;
        }
        if self.raw_mode {
            crossterm_terminal::disable_raw_mode()?;
            self.raw_mode = false;
        }
        Ok(())
    }
}

impl Drop for InteractiveSession {
    fn drop(&mut self) {
        let _ = self.terminal.finish();
        if self.bracketed_paste {
            let _ = execute!(io::stdout(), DisableBracketedPaste);
            self.bracketed_paste = false;
        }
        if self.raw_mode {
            let _ = crossterm_terminal::disable_raw_mode();
            self.raw_mode = false;
        }
    }
}

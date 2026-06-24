use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode as CrosstermKeyCode,
        KeyEvent as CrosstermKeyEvent, KeyEventKind, KeyModifiers as CrosstermKeyModifiers,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, terminal as crossterm_terminal,
};
use mural::{
    Color, Hr, KeyOutcome, Line, ListItem, Padding, Render, Size, Span, Spinner, StdoutBackend,
    Style, Terminal, Text, TextError, Textarea,
};

const FPS: u64 = 15;
const FRAME_DELAY: Duration = Duration::from_millis(1_000 / FPS);
const ID_INPUT: &str = "input";
const ID_TYPING: &str = "typing";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run this example manually in a real terminal:
    //
    //     cargo run --example interactive
    //
    // It uses crossterm for raw-mode keyboard events while Mural owns only the
    // normal-buffer rendering surface.
    let mut session = InteractiveSession::start()?;

    setup_ui(session.terminal_mut())?;
    run_event_loop(&mut session)?;
    session.finish()?;

    println!("finished: submitted messages remain above.");
    Ok(())
}

fn setup_ui(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal.push_pinned(Text::from_plain("")?)?;
    terminal.insert_pinned(ID_TYPING, pinned(TypingIndicator::new()?))?;
    terminal.push_pinned(separator())?;
    terminal.insert_pinned(
        ID_INPUT,
        pinned(
            Textarea::new()
                .placeholder("type something…")?
                .placeholder_style(Style::new().dim().italic())
                .prompt_style(Style::new().fg(Color::Cyan))
                .max_height(5),
        ),
    )?;
    terminal.push_pinned(separator())?;
    terminal.push_pinned(pinned(help_text()?))?;
    terminal.render()?;
    Ok(())
}

fn run_event_loop(session: &mut InteractiveSession) -> Result<(), Box<dyn std::error::Error>> {
    let mut dirty = false;
    let mut animate = false;
    let mut last_render = Instant::now();

    loop {
        let should_render = dirty || animate;
        if should_render && last_render.elapsed() >= FRAME_DELAY {
            session.terminal_mut().render()?;
            last_render = Instant::now();
            dirty = false;
        }

        let poll_timeout = if should_render {
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
                let (changed, typing) = handle_input_key(session.terminal_mut(), key)?;
                dirty |= changed;
                if let Some(typing) = typing {
                    animate = typing;
                }
            }
            Event::Resize(width, height) => {
                session.terminal_mut().resize(Size::new(width, height))?;
                dirty = true;
            }
            Event::Paste(text) => {
                animate = handle_paste(session.terminal_mut(), text)?;
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
) -> Result<(bool, Option<bool>), Box<dyn std::error::Error>> {
    let outcome = terminal
        .pinned_block_mut::<Padding<Textarea>>(ID_INPUT)?
        .content_mut()
        .handle_key_event(key);

    let typing = match outcome {
        KeyOutcome::Submit => Some(submit_input(terminal)?),
        KeyOutcome::Changed | KeyOutcome::Unchanged => Some(update_typing_indicator(terminal)?),
        KeyOutcome::Ignored => None,
    };

    Ok((!matches!(outcome, KeyOutcome::Ignored), typing))
}

fn handle_paste(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
    text: String,
) -> Result<bool, Box<dyn std::error::Error>> {
    terminal
        .pinned_block_mut::<Padding<Textarea>>(ID_INPUT)?
        .content_mut()
        .insert_str(text);
    update_typing_indicator(terminal)
}

fn submit_input(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let submitted = terminal
        .pinned_block_mut::<Padding<Textarea>>(ID_INPUT)?
        .content_mut()
        .take();
    if !submitted.is_empty() {
        terminal.push_live(submitted_message(&submitted)?)?;
    }

    update_typing_indicator(terminal)
}

fn update_typing_indicator(
    terminal: &mut Terminal<StdoutBackend<io::Stdout>>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let is_typing = !terminal
        .pinned_block_mut::<Padding<Textarea>>(ID_INPUT)?
        .content_mut()
        .value()
        .is_empty();
    terminal
        .pinned_block_mut::<Padding<TypingIndicator>>(ID_TYPING)?
        .content_mut()
        .set_visible(is_typing);
    Ok(is_typing)
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
        ListItem::new(Text::from_raw_lossy(content)?)
            .bullet("›")?
            .bullet_style(Style::new().fg(Color::BrightCyan).bold())
            .gap(1),
    )
    .top(1)
    .left(1))
}

fn pinned<T>(block: T) -> Padding<T> {
    Padding::new(block).left(1)
}

fn separator() -> Hr {
    Hr::new().style(Style::new().fg(Color::Cyan))
}

fn help_text() -> Result<Text, TextError> {
    styled_text(
        "Enter submit · Shift/Alt+Enter newline · Esc/Ctrl+C quit",
        Style::new().dim(),
    )
}

fn styled_text(content: &str, style: Style) -> Result<Text, TextError> {
    Ok(Text::from_lines(vec![Line::from_spans(vec![Span::new(
        content, style,
    )?])]))
}

struct TypingIndicator {
    spinner: Spinner,
    visible: bool,
}

impl TypingIndicator {
    fn new() -> Result<Self, TextError> {
        Ok(Self {
            spinner: Spinner::new(styled_text("Typing...", Style::new().dim().italic())?)
                .spinner_style(Style::new().fg(Color::BrightYellow)),
            visible: false,
        })
    }

    fn set_visible(&mut self, visible: bool) {
        if visible && !self.visible {
            self.spinner.reset();
        }
        self.visible = visible;
    }
}

impl Render for TypingIndicator {
    fn render(&self, width: u16) -> Text {
        if self.visible {
            self.spinner.render(width)
        } else {
            Text::empty()
        }
    }

    fn render_every_frame(&self) -> bool {
        self.visible && self.spinner.render_every_frame()
    }
}

struct InteractiveSession {
    terminal: Terminal<StdoutBackend<io::Stdout>>,
    raw_mode: bool,
    bracketed_paste: bool,
    keyboard_enhancement: bool,
}

impl InteractiveSession {
    fn start() -> io::Result<Self> {
        crossterm_terminal::enable_raw_mode()?;

        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnableBracketedPaste) {
            let _ = crossterm_terminal::disable_raw_mode();
            return Err(error);
        }

        let keyboard_enhancement = execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
        )
        .is_ok();

        match Terminal::stdout() {
            Ok(terminal) => Ok(Self {
                terminal,
                raw_mode: true,
                bracketed_paste: true,
                keyboard_enhancement,
            }),
            Err(error) => {
                if keyboard_enhancement {
                    let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
                }
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
        if self.keyboard_enhancement {
            execute!(io::stdout(), PopKeyboardEnhancementFlags)?;
            self.keyboard_enhancement = false;
        }
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
        if self.keyboard_enhancement {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
            self.keyboard_enhancement = false;
        }
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

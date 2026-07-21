use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use mural_core::{
    BackendProbe, ColorScheme, TerminaBackend, Terminal as MuralTerminal, TerminalSize,
};
use mural_core::{
    key::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyOutcome},
    widget::Textarea,
};
use termina::{
    Event, EventReader, PlatformTerminal, Terminal as _,
    escape::csi::{Csi, DecPrivateMode, DecPrivateModeCode, Mode},
};

const TEXTAREA_ID: &str = "textarea";
const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / 60);

fn main() -> Result<(), Box<dyn Error>> {
    let mut backend = TerminaBackend::new()?;
    backend.as_mut().enter_raw_mode()?;
    set_bracketed_paste(backend.as_mut(), true)?;
    let _bracketed_paste = BracketedPasteGuard;

    let events = backend.as_ref().event_reader();
    let size = backend.terminal_size()?;
    let position = backend.cursor_position()?;
    let color_scheme = backend.color_scheme()?.unwrap_or(ColorScheme::Dark);

    let mut terminal = MuralTerminal::new(backend, size, position, color_scheme)?;
    terminal.insert_pinned(TEXTAREA_ID, Textarea::new());
    terminal.push_pinned("\x1b[2mEnter submits · Shift/Alt+Enter adds a line · Esc exits\x1b[22m");

    let run_result = run(&mut terminal, &events);
    let finish_result = terminal.finish();

    run_result?;
    finish_result?;
    Ok(())
}

struct BracketedPasteGuard;

impl Drop for BracketedPasteGuard {
    fn drop(&mut self) {
        if let Ok(mut terminal) = PlatformTerminal::new() {
            let _ = set_bracketed_paste(&mut terminal, false);
        }
    }
}

fn set_bracketed_paste(terminal: &mut impl io::Write, enabled: bool) -> io::Result<()> {
    let mode = DecPrivateMode::Code(DecPrivateModeCode::BracketedPaste);
    let mode = if enabled {
        Mode::SetDecPrivateMode(mode)
    } else {
        Mode::ResetDecPrivateMode(mode)
    };

    write!(terminal, "{}", Csi::Mode(mode))?;
    terminal.flush()
}

fn run(
    terminal: &mut MuralTerminal<TerminaBackend>,
    events: &EventReader,
) -> Result<(), Box<dyn Error>> {
    let mut next_frame = Instant::now();

    loop {
        let now = Instant::now();
        if now >= next_frame {
            terminal.render()?;
            next_frame += FRAME_TIME;
            let rendered_at = Instant::now();
            if next_frame <= rendered_at {
                next_frame = rendered_at + FRAME_TIME;
            }
        }

        let timeout = next_frame.saturating_duration_since(Instant::now());
        if !events.poll(Some(timeout), |_| true)? {
            continue;
        }

        match events.read(|_| true)? {
            Event::Key(key) => {
                let key = KeyEvent::from(key);
                if should_quit(key) {
                    break;
                }

                let outcome = terminal
                    .get_pinned_mut::<Textarea>(TEXTAREA_ID)
                    .expect("the pinned textarea must exist")
                    .handle_key_event(key);
                if outcome == KeyOutcome::Submit {
                    let submitted = terminal
                        .get_pinned_mut::<Textarea>(TEXTAREA_ID)
                        .expect("the pinned textarea must exist")
                        .take();
                    terminal.push_live(submitted);
                }
            }
            Event::Paste(text) => {
                terminal
                    .get_pinned_mut::<Textarea>(TEXTAREA_ID)
                    .expect("the pinned textarea must exist")
                    .insert(text);
            }
            Event::WindowResized(size) if size.cols > 0 && size.rows > 0 => {
                terminal.resize(TerminalSize {
                    height: usize::from(size.rows),
                    width: usize::from(size.cols),
                })?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn should_quit(event: KeyEvent) -> bool {
    event.kind() != KeyEventKind::Release
        && (event.code() == KeyCode::Escape
            || (matches!(event.code(), KeyCode::Char('c' | 'C'))
                && event.modifiers().contains(KeyModifiers::CONTROL)))
}

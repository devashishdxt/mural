use brisk::{Render, Size, Terminal, Text};
use std::{cell::Cell, thread, time::Duration};

const FRAME_DELAY: Duration = Duration::from_millis(700);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run this example manually to watch Brisk update a conversation in the
    // terminal's normal buffer:
    //
    //     cargo run --example conversation
    let mut terminal = Terminal::stdout()?;

    // Live blocks are the transcript. Pinned blocks render after live blocks and
    // are useful for status, token counts, or progress indicators.
    terminal.push_live(Text::from_plain("user: explain Brisk in one sentence")?)?;
    terminal.insert_live("assistant", Text::from_plain("assistant: thinking…")?)?;
    terminal.insert_pinned("status", Status::queued())?;
    render_frame(&mut terminal)?;

    // Identified blocks can be mutated between renders. This simulates a
    // streaming assistant response over several visible frames. The pinned
    // status is switched to "working" once; after that, its spinner advances on
    // every render because Status opts into Render::render_every_frame().
    terminal.pinned_block_mut::<Status>("status")?.set_working();
    for content in [
        "assistant: Brisk keeps",
        "assistant: Brisk keeps a live conversation",
        "assistant: Brisk keeps a live conversation region plus pinned status",
        "assistant: Brisk keeps a live conversation region plus pinned status in a normal terminal buffer.",
    ] {
        *terminal.live_block_mut::<Text>("assistant")? = Text::from_plain(content)?;
        render_frame(&mut terminal)?;
    }

    // Multiple state changes may be batched before a single render call.
    terminal.push_live(Text::from_plain(
        "user: what happens if the terminal changes size?",
    )?)?;
    terminal
        .pinned_block_mut::<Status>("status")?
        .set_batching_resize();
    terminal.resize(Size::new(48, 12))?;
    render_frame(&mut terminal)?;

    terminal.insert_live(
        "assistant-resize",
        Text::from_plain(
            "assistant: the caller notifies Brisk, and the next render performs a full redraw at the new safe width.",
        )?,
    )?;
    terminal.pinned_block_mut::<Status>("status")?.set_done();
    render_frame(&mut terminal)?;

    // finish() removes pinned UI, leaves live transcript text behind, restores
    // the cursor, and flushes the backend.
    terminal.finish()?;
    println!("\nfinished: pinned status was cleaned up; live transcript remains above.");

    Ok(())
}

fn render_frame<B: brisk::Backend>(terminal: &mut Terminal<B>) -> std::io::Result<()> {
    terminal.render()?;
    thread::sleep(FRAME_DELAY);
    Ok(())
}

struct Status {
    state: StatusState,
    animation_frame: Cell<usize>,
}

impl Status {
    fn queued() -> Self {
        Self {
            state: StatusState::Queued,
            animation_frame: Cell::new(0),
        }
    }

    fn set_working(&mut self) {
        self.state = StatusState::Working;
    }

    fn set_batching_resize(&mut self) {
        self.state = StatusState::BatchingResize;
    }

    fn set_done(&mut self) {
        self.state = StatusState::Done;
    }
}

enum StatusState {
    Queued,
    Working,
    BatchingResize,
    Done,
}

impl Render for Status {
    fn render(&self, _width: u16) -> Text {
        match self.state {
            StatusState::Queued => Text::from_plain("status: queued").unwrap(),
            StatusState::Working => {
                let frame = self.animation_frame.get();
                self.animation_frame.set(frame + 1);
                let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
                    [frame % 10];
                Text::from_plain(format!("status: working {spinner}")).unwrap()
            }
            StatusState::BatchingResize => {
                Text::from_plain("status: batching resize notification").unwrap()
            }
            StatusState::Done => Text::from_plain("status: done").unwrap(),
        }
    }

    fn render_every_frame(&self) -> bool {
        matches!(self.state, StatusState::Working)
    }
}

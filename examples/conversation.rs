use brisk::{Size, Terminal, Text};
use std::{thread, time::Duration};

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
    terminal.insert_pinned("status", Text::from_plain("status: queued")?)?;
    render_frame(&mut terminal)?;

    // Identified blocks can be mutated between renders. This simulates a
    // streaming assistant response over several visible frames.
    for (tokens, content) in [
        (4, "assistant: Brisk keeps"),
        (8, "assistant: Brisk keeps a live conversation"),
        (
            13,
            "assistant: Brisk keeps a live conversation region plus pinned status",
        ),
        (
            18,
            "assistant: Brisk keeps a live conversation region plus pinned status in a normal terminal buffer.",
        ),
    ] {
        *terminal.live_block_mut::<Text>("assistant")? = Text::from_plain(content)?;
        *terminal.pinned_block_mut::<Text>("status")? =
            Text::from_plain(format!("status: streaming • {tokens} tokens"))?;
        render_frame(&mut terminal)?;
    }

    // Multiple state changes may be batched before a single render call.
    terminal.push_live(Text::from_plain(
        "user: what happens if the terminal changes size?",
    )?)?;
    *terminal.pinned_block_mut::<Text>("status")? =
        Text::from_plain("status: batching resize notification")?;
    terminal.resize(Size::new(48, 12))?;
    render_frame(&mut terminal)?;

    terminal.insert_live(
        "assistant-resize",
        Text::from_plain(
            "assistant: the caller notifies Brisk, and the next render performs a full redraw at the new safe width.",
        )?,
    )?;
    *terminal.pinned_block_mut::<Text>("status")? = Text::from_plain("status: done")?;
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

use brisk::{
    Color, Hr, ListItem, Padding, Size, Spinner, Style, Terminal, Text, list_item, spinner,
};
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
    terminal.push_live(padded(list_item("user: explain Brisk in one sentence")?))?;
    terminal.insert_live("assistant", padded(list_item("assistant: thinking…")?))?;
    terminal.push_pinned(padded(
        Hr::new().style(Style::new().fg(Color::BrightBlack).dim()),
    ))?;
    terminal.insert_pinned("status", padded(spinner("status: queued")?))?;
    render_frame(&mut terminal)?;

    // Identified blocks can be mutated between renders. This simulates a
    // streaming assistant response over several visible frames. The pinned
    // status spinner advances on every render while it is running.
    set_status(&mut terminal, "status: working")?;
    for content in [
        "assistant: Brisk keeps",
        "assistant: Brisk keeps a live conversation",
        "assistant: Brisk keeps a live conversation region plus pinned status",
        "assistant: Brisk keeps a live conversation region plus pinned status in a normal terminal buffer.",
    ] {
        *terminal
            .live_block_mut::<Padding<ListItem>>("assistant")?
            .content_mut() = list_item(content)?;
        render_frame(&mut terminal)?;
    }

    // Multiple state changes may be batched before a single render call.
    terminal.push_live(padded(list_item(
        "user: what happens if the terminal changes size?",
    )?))?;
    set_status(&mut terminal, "status: batching resize notification")?;
    terminal.resize(Size::new(48, 12))?;
    render_frame(&mut terminal)?;

    terminal.insert_live(
        "assistant-resize",
        padded(list_item(
            "assistant: the caller notifies Brisk, and the next render performs a full redraw at the new safe width.",
        )?),
    )?;
    complete_status(&mut terminal, "status: done")?;
    render_frame(&mut terminal)?;

    // finish() removes pinned UI, leaves live transcript text behind, restores
    // the cursor, and flushes the backend.
    terminal.finish()?;
    println!("\nfinished: pinned status was cleaned up; live transcript remains above.");

    Ok(())
}

fn padded<T>(block: T) -> Padding<T> {
    Padding::new(block).top(1).left(1)
}

fn render_frame<B: brisk::Backend>(terminal: &mut Terminal<B>) -> std::io::Result<()> {
    terminal.render()?;
    thread::sleep(FRAME_DELAY);
    Ok(())
}

fn set_status<B: brisk::Backend>(
    terminal: &mut Terminal<B>,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = terminal
        .pinned_block_mut::<Padding<Spinner>>("status")?
        .content_mut();
    *status.content_mut() = Text::from_plain(content)?;
    status.reset();
    Ok(())
}

fn complete_status<B: brisk::Backend>(
    terminal: &mut Terminal<B>,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = terminal
        .pinned_block_mut::<Padding<Spinner>>("status")?
        .content_mut();
    *status.content_mut() = Text::from_plain(content)?;
    status.succeed();
    Ok(())
}

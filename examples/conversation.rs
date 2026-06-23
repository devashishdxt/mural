use std::{thread, time::Duration};

use mural::{
    Color, Hr, Line, ListItem, Padding, Render, Size, Span, Spinner, Style, Terminal, Text,
    Textarea,
};

const FPS: u64 = 15;
const FRAME_DELAY: Duration = Duration::from_millis(1_000 / FPS);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run this example manually to watch Mural update a conversation in the
    // terminal's normal buffer:
    //
    //     cargo run --example conversation
    let mut terminal = Terminal::stdout()?;

    // Live blocks are the transcript. Pinned blocks render after live blocks and
    // are useful for transient status and input UI. The status block starts
    // hidden, then appears directly above the input separator while the
    // assistant is answering.
    terminal.push_pinned(Text::from_plain("")?)?;
    terminal.insert_pinned("status", pinned(AnswerStatus::hidden()))?;
    terminal.push_pinned(pinned(
        Hr::new().style(Style::new().fg(Color::BrightBlack).dim()),
    ))?;
    terminal.insert_pinned(
        "input",
        pinned(
            Textarea::new()
                .placeholder("type a message…")?
                .placeholder_style(Style::new().fg(Color::BrightBlack).dim())
                .max_height(3),
        ),
    )?;
    terminal.push_pinned(pinned(
        Hr::new().style(Style::new().fg(Color::BrightBlack).dim()),
    ))?;
    render_frames(&mut terminal, 8)?;

    // The first user message also goes through the textarea: applications own
    // raw mode and keyboard events; this example mutates the textarea directly
    // to simulate typing and submitting.
    type_into_input(&mut terminal, "explain Mural in one sentence")?;
    render_frames(&mut terminal, 4)?;
    submit_input(&mut terminal)?;

    show_status(&mut terminal, "answering")?;
    terminal.insert_live("thinking", live_padded(thinking_message("thinking…")?))?;
    render_frames(&mut terminal, 10)?;
    terminal.remove_live("thinking")?;
    terminal.insert_live("assistant", live_padded(assistant_message("Mural keeps")?))?;
    render_frames(&mut terminal, 3)?;

    // Identified blocks can be mutated between renders. This simulates a
    // streaming assistant response over several visible frames. The pinned
    // status spinner advances on every render while it is visible.
    for content in [
        "Mural keeps",
        "Mural keeps a live conversation",
        "Mural keeps a live conversation region plus pinned input",
        "Mural keeps a live conversation region plus pinned input/status UI in a normal terminal buffer.",
    ] {
        *terminal
            .live_block_mut::<Padding<ListItem>>("assistant")?
            .content_mut() = assistant_message(content)?;
        render_frames(&mut terminal, 5)?;
    }
    hide_status(&mut terminal)?;
    render_frames(&mut terminal, 6)?;

    // Type a second message, move the cursor left, insert a missing word, then
    // submit. The submitted value is appended to the live transcript.
    type_into_input(&mut terminal, "what happens if terminal changes size?")?;
    move_input_left(&mut terminal, "terminal changes size?".chars().count())?;
    type_into_input(&mut terminal, "the ")?;
    render_frames(&mut terminal, 6)?;

    let submitted = submit_input(&mut terminal)?;
    show_status(&mut terminal, "adapting to resize")?;
    terminal.insert_live(
        "thinking-resize",
        live_padded(thinking_message("checking terminal size…")?),
    )?;
    render_frames(&mut terminal, 8)?;

    terminal.resize(Size::new(48, 12))?;
    *terminal
        .live_block_mut::<Padding<ListItem>>("thinking-resize")?
        .content_mut() = thinking_message("reflowing the conversation for 48 columns…")?;
    render_frames(&mut terminal, 10)?;
    terminal.remove_live("thinking-resize")?;
    terminal.insert_live(
        "assistant-resize",
        live_padded(assistant_message(format!(
            "For `{submitted}`, the caller notifies Mural about the new size, and the next render performs a full redraw at the new safe width."
        ))?),
    )?;
    render_frames(&mut terminal, 12)?;
    hide_status(&mut terminal)?;
    render_frames(&mut terminal, 6)?;

    // finish() removes pinned UI, leaves live transcript text behind, restores
    // the cursor, and flushes the backend.
    terminal.finish()?;
    println!("\nfinished: pinned status/input were cleaned up; live transcript remains above.");

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnswerStatus {
    spinner: Spinner,
    visible: bool,
}

impl AnswerStatus {
    fn hidden() -> Self {
        Self {
            spinner: Spinner::new(Text::from_plain("answering").unwrap())
                .spinner_style(Style::new().fg(Color::BrightBlack).dim()),
            visible: false,
        }
    }

    fn show(&mut self, content: &str) -> Result<&mut Self, mural::TextError> {
        *self.spinner.content_mut() = Text::from_plain(content)?;
        self.spinner.reset();
        self.visible = true;
        Ok(self)
    }

    fn hide(&mut self) -> &mut Self {
        self.visible = false;
        self
    }
}

impl Render for AnswerStatus {
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

fn user_message(content: impl AsRef<str>) -> Result<ListItem, mural::TextError> {
    Ok(ListItem::new(styled_text(content, Style::new()))
        .bullet("›")?
        .bullet_style(Style::new().fg(Color::BrightCyan).bold())
        .gap(1))
}

fn assistant_message(content: impl AsRef<str>) -> Result<ListItem, mural::TextError> {
    Ok(ListItem::new(styled_text(content, Style::new()))
        .bullet("✦")?
        .bullet_style(Style::new().fg(Color::BrightMagenta).bold())
        .gap(1))
}

fn thinking_message(content: impl AsRef<str>) -> Result<ListItem, mural::TextError> {
    Ok(ListItem::new(styled_text(
        content,
        Style::new().fg(Color::BrightBlack).dim(),
    ))
    .bullet("·")?
    .bullet_style(Style::new().fg(Color::BrightBlack).dim())
    .gap(1))
}

fn styled_text(content: impl AsRef<str>, style: Style) -> Text {
    Text::from_lines(vec![Line::from_spans(vec![
        Span::new(content.as_ref(), style).expect("example message text is valid plain content"),
    ])])
}

fn live_padded<T>(block: T) -> Padding<T> {
    Padding::new(block).top(1).left(1)
}

fn pinned<T>(block: T) -> Padding<T> {
    Padding::new(block).left(1)
}

fn render_frame<B: mural::Backend>(terminal: &mut Terminal<B>) -> std::io::Result<()> {
    terminal.render()?;
    thread::sleep(FRAME_DELAY);
    Ok(())
}

fn render_frames<B: mural::Backend>(
    terminal: &mut Terminal<B>,
    frames: usize,
) -> std::io::Result<()> {
    for _ in 0..frames {
        render_frame(terminal)?;
    }
    Ok(())
}

fn type_into_input<B: mural::Backend>(
    terminal: &mut Terminal<B>,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for ch in content.chars() {
        terminal
            .pinned_block_mut::<Padding<Textarea>>("input")?
            .content_mut()
            .insert_char(ch);
        render_frame(terminal)?;
    }
    Ok(())
}

fn move_input_left<B: mural::Backend>(
    terminal: &mut Terminal<B>,
    steps: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..steps {
        terminal
            .pinned_block_mut::<Padding<Textarea>>("input")?
            .content_mut()
            .move_left();
        render_frame(terminal)?;
    }
    Ok(())
}

fn submit_input<B: mural::Backend>(
    terminal: &mut Terminal<B>,
) -> Result<String, Box<dyn std::error::Error>> {
    let submitted = {
        terminal
            .pinned_block_mut::<Padding<Textarea>>("input")?
            .content_mut()
            .take()
    };

    terminal.push_live(live_padded(user_message(&submitted)?))?;
    render_frame(terminal)?;
    Ok(submitted)
}

fn show_status<B: mural::Backend>(
    terminal: &mut Terminal<B>,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal
        .pinned_block_mut::<Padding<AnswerStatus>>("status")?
        .content_mut()
        .show(content)?;
    Ok(())
}

fn hide_status<B: mural::Backend>(
    terminal: &mut Terminal<B>,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal
        .pinned_block_mut::<Padding<AnswerStatus>>("status")?
        .content_mut()
        .hide();
    Ok(())
}

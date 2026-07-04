use crate::region::Region;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CommittedFrame {
    pub(super) lines: Vec<String>,
    pub(super) sentinel_row: usize,
}

pub(super) fn current_frame(
    live_blocks: &mut Region,
    pinned_blocks: &mut Region,
    width: usize,
) -> Vec<String> {
    let mut lines = live_blocks.render_lines(width);
    lines.extend(pinned_blocks.render_lines(width));
    lines
}

pub(super) fn frame_changed(last_frame: &[String], current_frame: &[String]) -> bool {
    last_frame.len() != current_frame.len()
        || last_frame
            .iter()
            .zip(current_frame)
            .any(|(last, current)| last != current)
}

pub(super) fn is_append_only(last_frame: &[String], current_frame: &[String]) -> bool {
    current_frame.len() > last_frame.len()
        && current_frame
            .iter()
            .zip(last_frame)
            .all(|(current, last)| current == last)
}

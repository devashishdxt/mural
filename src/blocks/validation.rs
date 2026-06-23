use unicode_width::UnicodeWidthStr;

use crate::{Span, TextError};

pub(super) fn validate_non_empty_display_text(content: &str) -> Result<usize, TextError> {
    Span::validate_content(content)?;

    let width = UnicodeWidthStr::width(content);
    if width == 0 {
        return Err(TextError::StructuralContent);
    }

    Ok(width)
}

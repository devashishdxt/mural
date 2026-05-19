use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextError {
    message: &'static str,
}

impl TextError {
    pub(crate) fn structural_content() -> Self {
        Self {
            message: "text contains unsupported structural content",
        }
    }
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl std::error::Error for TextError {}

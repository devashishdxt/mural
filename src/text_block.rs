use crate::Render;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    content: String,
}

impl TextBlock {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

impl Render for TextBlock {
    fn render(&self, width: u16) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let width = usize::from(width);
        self.content
            .split('\n')
            .flat_map(|line| {
                if line.is_empty() {
                    return vec![String::new()];
                }

                let mut rendered = Vec::new();
                let mut current = String::new();
                let mut current_width = 0;

                for ch in line.chars() {
                    current.push(ch);
                    current_width += 1;
                    if current_width == width {
                        rendered.push(std::mem::take(&mut current));
                        current_width = 0;
                    }
                }

                if !current.is_empty() {
                    rendered.push(current);
                }

                rendered
            })
            .collect()
    }
}

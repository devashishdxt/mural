use std::{borrow::Cow, iter::repeat_n};

use crate::{Block, Color, RenderContext};

pub struct Textarea {
    dark_rule_color: Option<Color>,
    light_rule_color: Option<Color>,
}

impl Block for Textarea {
    fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>> {
        let hr: String = repeat_n('─', context.width()).collect();
        vec![Cow::Owned(hr.clone()), Cow::Borrowed(""), Cow::Owned(hr)]
    }
}

use brisk::{
    Color, Hr, Line, ListItem, Padding, Render, Size, Span, Style, Terminal, Text, TextError,
    backend::fake::{FakeBackend, Operation},
    blocks, hr, list_item, padding,
};

#[test]
fn hr_convenience_and_default_match_new() {
    assert_eq!(hr(), Hr::new());
    assert_eq!(Hr::default(), Hr::new());
    assert_eq!(blocks::hr(), Hr::new());
    assert_eq!(blocks::Hr::new(), Hr::new());
}

#[test]
fn default_hr_renders_a_full_width_plain_rule() {
    let text = Hr::new().render(5);

    assert_eq!(text.lines().len(), 1);
    assert_eq!(text.lines()[0], Line::from_plain("─────").unwrap());
}

#[test]
fn custom_character_renders_as_many_glyphs_as_fit() {
    let hr = Hr::with_character('界').unwrap();

    let text = hr.render(5);

    assert_eq!(hr.fill_character(), '界');
    assert_eq!(hr.character_width(), 2);
    assert_eq!(text.lines().len(), 1);
    assert_eq!(text.lines()[0].plain_content(), "界界");
    assert_eq!(text.lines()[0].display_width(), 4);
}

#[test]
fn character_wider_than_render_width_renders_one_blank_line() {
    let text = Hr::with_character('界').unwrap().render(1);

    assert_eq!(text.lines().len(), 1);
    assert!(text.lines()[0].spans().is_empty());
}

#[test]
fn zero_render_width_renders_no_lines() {
    let text = Hr::new().render(0);

    assert!(text.lines().is_empty());
}

#[test]
fn styled_rule_preserves_style() {
    let style = Style::new().fg(Color::BrightBlack).dim();
    let text = Hr::new().character('═').unwrap().style(style).render(3);

    assert_eq!(text.lines().len(), 1);
    assert_eq!(
        text.lines()[0],
        Line::from_spans(vec![Span::new("═══", style).unwrap()])
    );
    assert_eq!(Hr::new().style(style).rule_style(), style);
}

#[test]
fn invalid_characters_are_rejected_as_structural_content() {
    assert_eq!(
        Hr::with_character('\n').unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Hr::with_character('\t').unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Hr::with_character('\u{1b}').unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Hr::with_character('\u{0301}').unwrap_err(),
        TextError::StructuralContent
    );
}

#[test]
fn terminal_renders_hr_as_a_normal_block() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(6, 24))).unwrap();
    terminal.push_live(hr()).unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("─────").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn list_item_convenience_and_defaults_match_new() {
    let content = Text::from_plain("hello").unwrap();
    let item = ListItem::new(content.clone());

    assert_eq!(list_item("hello").unwrap(), item);
    assert_eq!(blocks::list_item("hello").unwrap(), item);
    assert_eq!(blocks::ListItem::new(content.clone()), item);
    assert_eq!(item.content(), &content);
    assert_eq!(item.bullet_content(), "•");
    assert_eq!(item.bullet_width(), 1);
    assert_eq!(item.gap_width(), 1);
    assert_eq!(item.bullet_style_value(), Style::new());
}

#[test]
fn list_item_wraps_content_with_hanging_indent() {
    let text = ListItem::new(Text::from_plain("abcdef").unwrap()).render(5);

    assert_eq!(text.lines().len(), 2);
    assert_eq!(text.lines()[0].plain_content(), "• abc");
    assert_eq!(text.lines()[1].plain_content(), "  def");
    assert!(text.lines().iter().all(|line| line.display_width() <= 5));
}

#[test]
fn list_item_preserves_explicit_blank_lines_with_indent() {
    let text = ListItem::new(Text::from_plain("first\n\nthird").unwrap()).render(20);

    assert_eq!(text.lines().len(), 3);
    assert_eq!(text.lines()[0].plain_content(), "• first");
    assert_eq!(text.lines()[1].plain_content(), "  ");
    assert_eq!(text.lines()[2].plain_content(), "  third");
}

#[test]
fn custom_list_item_bullet_gap_and_style_are_rendered() {
    let bullet_style = Style::new().fg(Color::BrightBlack).dim();
    let content_style = Style::new().fg(Color::Red);
    let content = Text::from_lines(vec![Line::from_spans(vec![
        Span::new("red", content_style).unwrap(),
        Span::plain(" text").unwrap(),
    ])]);

    let item = ListItem::new(content)
        .bullet("->")
        .unwrap()
        .bullet_style(bullet_style)
        .gap(2);
    let text = item.render(20);

    assert_eq!(item.bullet_content(), "->");
    assert_eq!(item.bullet_width(), 2);
    assert_eq!(item.gap_width(), 2);
    assert_eq!(item.bullet_style_value(), bullet_style);
    assert_eq!(
        text.lines()[0],
        Line::from_spans(vec![
            Span::new("->", bullet_style).unwrap(),
            Span::plain("  ").unwrap(),
            Span::new("red", content_style).unwrap(),
            Span::plain(" text").unwrap(),
        ])
    );
}

#[test]
fn list_item_handles_empty_content_and_narrow_widths() {
    assert!(ListItem::new(Text::empty()).render(10).lines().is_empty());
    assert_eq!(
        ListItem::new(Text::from_plain("").unwrap())
            .render(10)
            .lines()[0]
            .plain_content(),
        "• "
    );

    let item = ListItem::new(Text::from_plain("x").unwrap());
    assert!(item.render(0).lines().is_empty());
    assert_eq!(item.render(1).lines()[0].plain_content(), "•");
    assert_eq!(item.render(2).lines()[0].plain_content(), "• ");
    assert_eq!(item.render(3).lines()[0].plain_content(), "• x");

    let wide_bullet = ListItem::new(Text::from_plain("x").unwrap())
        .bullet("界")
        .unwrap();
    assert!(wide_bullet.render(1).lines().is_empty());
    assert_eq!(wide_bullet.render(2).lines()[0].plain_content(), "界");
}

#[test]
fn generic_list_item_wraps_any_render_block_and_forwards_frame_hint() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RenderedText {
        text: Text,
        every_frame: bool,
    }

    impl Render for RenderedText {
        fn render(&self, width: u16) -> Text {
            self.text.render(width)
        }

        fn render_every_frame(&self) -> bool {
            self.every_frame
        }
    }

    let mut item = ListItem::new(RenderedText {
        text: Text::from_plain("abcdef").unwrap(),
        every_frame: true,
    });
    item.content_mut().text = Text::from_plain("abcdefghi").unwrap();

    let text = item.render(5);

    assert!(item.render_every_frame());
    assert_eq!(text.lines()[0].plain_content(), "• abc");
    assert_eq!(text.lines()[1].plain_content(), "  def");
    assert_eq!(text.lines()[2].plain_content(), "  ghi");
}

#[test]
fn invalid_list_item_bullets_are_rejected_as_structural_content() {
    let item = ListItem::new(Text::from_plain("hello").unwrap());

    assert_eq!(
        item.clone().bullet("").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        item.clone().bullet("\n").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        item.clone().bullet("\t").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        item.clone().bullet("\u{1b}").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        item.bullet("\u{0301}").unwrap_err(),
        TextError::StructuralContent
    );
}

#[test]
fn padding_convenience_and_defaults_match_new() {
    let content = Text::from_plain("hello").unwrap();
    let block = Padding::new(content.clone());

    assert_eq!(padding(content.clone()), block);
    assert_eq!(blocks::padding(content.clone()), block);
    assert_eq!(blocks::Padding::new(content.clone()), block);
    assert_eq!(block.content(), &content);
    assert_eq!(block.top_height(), 0);
    assert_eq!(block.bottom_height(), 0);
    assert_eq!(block.left_width(), 0);
    assert_eq!(block.right_width(), 0);
}

#[test]
fn padding_builders_and_getters_set_each_side() {
    let block = Padding::new(Text::from_plain("hello").unwrap())
        .top(1)
        .bottom(2)
        .left(3)
        .right(4);

    assert_eq!(block.top_height(), 1);
    assert_eq!(block.bottom_height(), 2);
    assert_eq!(block.left_width(), 3);
    assert_eq!(block.right_width(), 4);

    let vertical = Padding::new(Text::empty()).vertical(5);
    assert_eq!(vertical.top_height(), 5);
    assert_eq!(vertical.bottom_height(), 5);

    let horizontal = Padding::new(Text::empty()).horizontal(6);
    assert_eq!(horizontal.left_width(), 6);
    assert_eq!(horizontal.right_width(), 6);

    let all = Padding::new(Text::empty()).all(7);
    assert_eq!(all.top_height(), 7);
    assert_eq!(all.bottom_height(), 7);
    assert_eq!(all.left_width(), 7);
    assert_eq!(all.right_width(), 7);
}

#[test]
fn padding_wraps_content_inside_horizontal_padding() {
    let text = Padding::new(Text::from_plain("abcdef").unwrap())
        .left(2)
        .right(3)
        .render(10);

    assert_eq!(text.lines().len(), 2);
    assert_eq!(text.lines()[0].plain_content(), "  abcde");
    assert_eq!(text.lines()[1].plain_content(), "  f");
    assert!(text.lines().iter().all(|line| line.display_width() <= 10));
}

#[test]
fn padding_does_not_emit_right_padding_spaces() {
    let text = Padding::new(Text::from_plain("abc").unwrap())
        .left(2)
        .right(3)
        .render(10);

    assert_eq!(text.lines().len(), 1);
    assert_eq!(text.lines()[0].plain_content(), "  abc");
    assert_eq!(text.lines()[0].display_width(), 5);
}

#[test]
fn padding_top_and_bottom_are_empty_lines() {
    let text = Padding::new(Text::from_plain("body").unwrap())
        .top(2)
        .bottom(1)
        .render(10);

    assert_eq!(text.lines().len(), 4);
    assert!(text.lines()[0].spans().is_empty());
    assert!(text.lines()[1].spans().is_empty());
    assert_eq!(text.lines()[2].plain_content(), "body");
    assert!(text.lines()[3].spans().is_empty());
}

#[test]
fn padding_preserves_vertical_padding_around_empty_content() {
    let text = Padding::new(Text::empty()).top(1).bottom(2).render(10);

    assert_eq!(text.lines().len(), 3);
    assert!(text.lines().iter().all(|line| line.spans().is_empty()));
}

#[test]
fn padding_applies_left_padding_to_explicit_blank_content_lines() {
    let text = Padding::new(Text::from_plain("a\n\nb").unwrap())
        .left(2)
        .render(10);

    assert_eq!(text.lines().len(), 3);
    assert_eq!(text.lines()[0].plain_content(), "  a");
    assert_eq!(text.lines()[1].plain_content(), "  ");
    assert_eq!(text.lines()[2].plain_content(), "  b");
}

#[test]
fn padding_handles_zero_and_narrow_widths() {
    let block = Padding::new(Text::from_plain("abc").unwrap())
        .top(1)
        .bottom(1)
        .left(2)
        .right(2);

    assert!(block.render(0).lines().is_empty());

    let text = block.render(3);
    assert_eq!(text.lines().len(), 2);
    assert!(text.lines()[0].spans().is_empty());
    assert!(text.lines()[1].spans().is_empty());
}

#[test]
fn padding_handles_huge_padding_values_without_overflow() {
    let text = Padding::new(Text::from_plain("abc").unwrap())
        .left(usize::MAX)
        .right(usize::MAX)
        .top(1)
        .render(5);

    assert_eq!(text.lines().len(), 1);
    assert!(text.lines()[0].spans().is_empty());
}

#[test]
fn generic_padding_wraps_any_render_block_and_forwards_frame_hint() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RenderedText {
        text: Text,
        every_frame: bool,
    }

    impl Render for RenderedText {
        fn render(&self, width: u16) -> Text {
            self.text.render(width)
        }

        fn render_every_frame(&self) -> bool {
            self.every_frame
        }
    }

    let mut block = Padding::new(RenderedText {
        text: Text::from_plain("abcdef").unwrap(),
        every_frame: true,
    })
    .left(1)
    .right(1);
    block.content_mut().text = Text::from_plain("abcdefghi").unwrap();

    let text = block.render(5);

    assert!(block.render_every_frame());
    assert_eq!(text.lines()[0].plain_content(), " abc");
    assert_eq!(text.lines()[1].plain_content(), " def");
    assert_eq!(text.lines()[2].plain_content(), " ghi");
}

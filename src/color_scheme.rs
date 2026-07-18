use std::env;

/// The terminal's preferred color scheme.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// A light terminal background.
    Light,

    /// A dark terminal background.
    #[default]
    Dark,
}

impl ColorScheme {
    pub(crate) fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        let luminance = 0.2126 * linear_channel(red)
            + 0.7152 * linear_channel(green)
            + 0.0722 * linear_channel(blue);

        if luminance >= 0.5 {
            Self::Light
        } else {
            Self::Dark
        }
    }
}

pub(crate) fn detect_from_env() -> Option<ColorScheme> {
    env::var("COLORFGBG")
        .ok()
        .as_deref()
        .and_then(detect_from_colorfgbg)
}

fn detect_from_colorfgbg(value: &str) -> Option<ColorScheme> {
    value
        .split(';')
        .rev()
        .find_map(|part| part.trim().parse::<u8>().ok())
        .map(ansi_color_scheme)
}

fn ansi_color_scheme(index: u8) -> ColorScheme {
    let (red, green, blue) = ansi_rgb(index);
    ColorScheme::from_rgb(red, green, blue)
}

fn ansi_rgb(index: u8) -> (u8, u8, u8) {
    const BASIC_COLORS: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];

    match index {
        0..=15 => BASIC_COLORS[usize::from(index)],
        16..=231 => {
            let cube_index = index - 16;
            (
                color_cube_channel(cube_index / 36),
                color_cube_channel((cube_index % 36) / 6),
                color_cube_channel(cube_index % 6),
            )
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    }
}

fn color_cube_channel(level: u8) -> u8 {
    if level == 0 { 0 } else { 55 + level * 40 }
}

fn linear_channel(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;

    if value <= 0.03928 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use super::{ColorScheme, ansi_rgb, detect_from_colorfgbg};

    #[test]
    fn dark_is_the_default_scheme() {
        assert_eq!(ColorScheme::default(), ColorScheme::Dark);
    }

    #[test]
    fn rgb_uses_wcag_relative_luminance() {
        assert_eq!(ColorScheme::from_rgb(0, 0, 0), ColorScheme::Dark);
        assert_eq!(ColorScheme::from_rgb(255, 255, 255), ColorScheme::Light);
        assert_eq!(ColorScheme::from_rgb(187, 187, 187), ColorScheme::Dark);
        assert_eq!(ColorScheme::from_rgb(188, 188, 188), ColorScheme::Light);
    }

    #[test]
    fn ansi_palette_converts_basic_cube_and_grayscale_colors() {
        assert_eq!(ansi_rgb(0), (0, 0, 0));
        assert_eq!(ansi_rgb(15), (255, 255, 255));
        assert_eq!(ansi_rgb(16), (0, 0, 0));
        assert_eq!(ansi_rgb(21), (0, 0, 255));
        assert_eq!(ansi_rgb(231), (255, 255, 255));
        assert_eq!(ansi_rgb(232), (8, 8, 8));
        assert_eq!(ansi_rgb(255), (238, 238, 238));
    }

    #[test]
    fn colorfgbg_uses_the_last_valid_ansi_index() {
        assert_eq!(detect_from_colorfgbg("15;0"), Some(ColorScheme::Dark));
        assert_eq!(
            detect_from_colorfgbg("0;invalid;15;also-invalid"),
            Some(ColorScheme::Light)
        );
        assert_eq!(detect_from_colorfgbg(" 0 ; 255 "), Some(ColorScheme::Light));
        assert_eq!(detect_from_colorfgbg(""), None);
        assert_eq!(detect_from_colorfgbg("invalid;256"), None);
    }
}

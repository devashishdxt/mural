use std::{
    io::{self, Write},
    time::Duration,
};

use termina::{
    Event, PlatformTerminal, Terminal as _,
    escape::{
        csi::{
            Csi, Cursor, DecPrivateMode, DecPrivateModeCode, Edit, EraseInDisplay, EraseInLine,
            Mode, ThemeMode,
        },
        osc::{ColorOrQuery, DynamicColorNumber, Osc},
    },
    style::RgbColor,
};

use crate::{
    backend::{Backend, BackendProbe},
    color_scheme::{ColorScheme, detect_from_env},
    terminal::{CursorPosition, TerminalSize},
};

const COLOR_SCHEME_QUERY_TIMEOUT: Duration = Duration::from_millis(100);

/// A terminal backend using Termina's platform terminal.
///
/// Color-scheme detection checks the terminal's default background color, then its operating-system
/// theme preference, then `COLORFGBG`. Each terminal query waits up to 100 milliseconds for a
/// matching response.
pub struct TerminaBackend {
    terminal: PlatformTerminal,
}

impl TerminaBackend {
    pub fn new() -> Result<Self, io::Error> {
        PlatformTerminal::new().map(Into::into)
    }

    pub fn into_inner(self) -> PlatformTerminal {
        self.terminal
    }

    fn write_csi(&mut self, csi: Csi) -> Result<(), io::Error> {
        write!(self.terminal, "{csi}")
    }

    fn write_osc(&mut self, osc: Osc<'_>) -> Result<(), io::Error> {
        write!(self.terminal, "{osc}")
    }

    fn query_terminal_color_scheme(&mut self) -> Result<Option<ColorScheme>, io::Error> {
        self.write_csi(Csi::Mode(Mode::QueryTheme))?;
        self.terminal.flush()?;

        if !self.terminal.poll(
            |event| color_scheme_from_event(event).is_some(),
            Some(COLOR_SCHEME_QUERY_TIMEOUT),
        )? {
            return Ok(None);
        }

        let event = self
            .terminal
            .read(|event| color_scheme_from_event(event).is_some())?;
        Ok(color_scheme_from_event(&event))
    }

    fn query_terminal_background(&mut self) -> Result<Option<RgbColor>, io::Error> {
        self.write_osc(Osc::ChangeDynamicColors(
            DynamicColorNumber::TextBackgroundColor,
            vec![ColorOrQuery::Query],
        ))?;
        self.terminal.flush()?;

        if !self.terminal.poll(
            |event| background_color_from_event(event).is_some(),
            Some(COLOR_SCHEME_QUERY_TIMEOUT),
        )? {
            return Ok(None);
        }

        let event = self
            .terminal
            .read(|event| background_color_from_event(event).is_some())?;
        Ok(background_color_from_event(&event))
    }
}

impl From<PlatformTerminal> for TerminaBackend {
    fn from(terminal: PlatformTerminal) -> Self {
        Self { terminal }
    }
}

impl AsRef<PlatformTerminal> for TerminaBackend {
    fn as_ref(&self) -> &PlatformTerminal {
        &self.terminal
    }
}

impl AsMut<PlatformTerminal> for TerminaBackend {
    fn as_mut(&mut self) -> &mut PlatformTerminal {
        &mut self.terminal
    }
}

impl Backend for TerminaBackend {
    type Error = io::Error;

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
            DecPrivateModeCode::ShowCursor,
        ))))
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
            DecPrivateModeCode::ShowCursor,
        ))))
    }

    fn move_up(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Cursor(Cursor::Up(n)))
    }

    fn move_down(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Cursor(Cursor::Down(n)))
    }

    fn carriage_return(&mut self) -> Result<(), Self::Error> {
        self.terminal.write_all(b"\r")
    }

    fn newline(&mut self) -> Result<(), Self::Error> {
        self.terminal.write_all(b"\n")
    }

    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Edit(Edit::ScrollUp(n)))
    }

    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Edit(Edit::InsertLine(n)))
    }

    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Edit(Edit::DeleteLine(n)))
    }

    fn clear_line(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Edit(Edit::EraseInLine(EraseInLine::EraseLine)))
    }

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        self.terminal.write_all(text.as_bytes())
    }

    fn clear_screen(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Edit(Edit::EraseInDisplay(
            EraseInDisplay::EraseDisplay,
        )))
    }

    fn purge_scrollback(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Edit(Edit::EraseInDisplay(
            EraseInDisplay::EraseScrollback,
        )))
    }

    fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Cursor(Cursor::default_position()))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.terminal.flush()
    }
}

/// The underlying terminal must be configured so terminal responses can be read before calling
/// probes that issue terminal queries, which typically means entering raw mode. Querying flushes
/// pending output, and unrelated events remain available to Termina's event reader. Callers must
/// not issue overlapping terminal queries.
impl BackendProbe for TerminaBackend {
    fn terminal_size(&mut self) -> Result<TerminalSize, Self::Error> {
        let size = self.terminal.get_dimensions()?;

        Ok(TerminalSize {
            height: usize::from(size.rows),
            width: usize::from(size.cols),
        })
    }

    fn cursor_position(&mut self) -> Result<CursorPosition, Self::Error> {
        self.write_csi(Csi::Cursor(Cursor::RequestActivePositionReport))?;
        self.terminal.flush()?;

        let event = self.terminal.read(|event| {
            matches!(
                event,
                Event::Csi(Csi::Cursor(Cursor::ActivePositionReport { .. }))
            )
        })?;
        let Event::Csi(Csi::Cursor(Cursor::ActivePositionReport { line, col })) = event else {
            unreachable!("filtered terminal read returned an unrelated event")
        };

        Ok(CursorPosition {
            row: usize::from(line.get_zero_based()),
            column: usize::from(col.get_zero_based()),
        })
    }

    fn color_scheme(&mut self) -> Result<Option<ColorScheme>, Self::Error> {
        if let Some(color) = self.query_terminal_background()? {
            return Ok(Some(ColorScheme::from_rgb(
                color.red,
                color.green,
                color.blue,
            )));
        }

        if let Some(scheme) = self.query_terminal_color_scheme()? {
            return Ok(Some(scheme));
        }

        Ok(detect_from_env())
    }
}

fn color_scheme_from_event(event: &Event) -> Option<ColorScheme> {
    match event {
        Event::Csi(Csi::Mode(Mode::ReportTheme(ThemeMode::Light))) => Some(ColorScheme::Light),
        Event::Csi(Csi::Mode(Mode::ReportTheme(ThemeMode::Dark))) => Some(ColorScheme::Dark),
        _ => None,
    }
}

fn background_color_from_event(event: &Event) -> Option<RgbColor> {
    let Event::Osc(Osc::ChangeDynamicColors(DynamicColorNumber::TextBackgroundColor, colors)) =
        event
    else {
        return None;
    };
    let [ColorOrQuery::Color(color)] = colors.as_slice() else {
        return None;
    };

    Some(*color)
}

fn nonzero_count(n: usize) -> Option<u32> {
    (n != 0).then(|| n.try_into().unwrap_or(u32::MAX))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use termina::{
        Event,
        escape::{
            csi::{Csi, Mode, ThemeMode},
            osc::{ColorOrQuery, DynamicColorNumber, Osc},
        },
        style::RgbColor,
    };

    use super::{background_color_from_event, color_scheme_from_event, nonzero_count};
    use crate::ColorScheme;

    #[test]
    fn count_omits_zero_and_clamps_large_values() {
        assert_eq!(nonzero_count(0), None);
        assert_eq!(nonzero_count(42), Some(42));
        assert_eq!(nonzero_count(usize::MAX), Some(u32::MAX));
    }

    #[test]
    fn color_scheme_reports_map_to_public_schemes() {
        assert_eq!(
            color_scheme_from_event(&Event::Csi(Csi::Mode(Mode::ReportTheme(ThemeMode::Dark)))),
            Some(ColorScheme::Dark)
        );
        assert_eq!(
            color_scheme_from_event(&Event::Csi(Csi::Mode(Mode::ReportTheme(ThemeMode::Light)))),
            Some(ColorScheme::Light)
        );
        assert_eq!(color_scheme_from_event(&Event::FocusIn), None);
    }

    #[test]
    fn background_color_requires_an_osc_11_color_response() {
        let color = RgbColor::new(40, 80, 120);
        let response = Event::Osc(Osc::ChangeDynamicColors(
            DynamicColorNumber::TextBackgroundColor,
            vec![ColorOrQuery::Color(color)],
        ));
        assert_eq!(background_color_from_event(&response), Some(color));

        let query = Event::Osc(Osc::ChangeDynamicColors(
            DynamicColorNumber::TextBackgroundColor,
            vec![ColorOrQuery::Query],
        ));
        assert_eq!(background_color_from_event(&query), None);

        let foreground = Event::Osc(Osc::ChangeDynamicColors(
            DynamicColorNumber::TextForegroundColor,
            vec![ColorOrQuery::Color(color)],
        ));
        assert_eq!(background_color_from_event(&foreground), None);
    }
}

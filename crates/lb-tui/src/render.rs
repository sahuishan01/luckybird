use lb_core::{Buffer, Selection};
use ratatui::{
    buffer::Buffer as RatBuf,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::statusline::StatusLine;

// ─── Data types consumed by the render layer ────────────────────────────────

/// A single syntax-highlight span: byte range + capture index.
/// Produced by lb-syntax (LB-0005); ignored until then.
#[derive(Debug, Clone)]
pub struct Highlight {
    pub start: usize,
    pub end: usize,
    pub capture: u32,
}

/// LSP diagnostic attached to a byte range.
/// Produced by lb-lsp (LB-0006); ignored until then.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub start: usize,
    pub end: usize,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

// ─── View ────────────────────────────────────────────────────────────────────

/// Everything the render layer needs for one frame. Built per-frame by lb-app;
/// never stored across frames.
pub struct View<'a> {
    pub buffer: &'a Buffer,
    pub selection: &'a Selection,
    /// Syntax highlights (empty until lb-syntax is wired in).
    pub highlights: &'a [Highlight],
    /// LSP diagnostics (empty until lb-lsp is wired in).
    pub diagnostics: &'a [Diagnostic],
    pub status: StatusLine,
    /// Index of the first visible line (0-based).
    pub scroll_offset: usize,
}

// ─── Editor widget ───────────────────────────────────────────────────────────

/// A full-screen editor widget (buffer + status line).
/// Split into a gutter, text area, and a 1-row status bar.
pub struct EditorWidget<'a> {
    view: &'a View<'a>,
}

impl<'a> EditorWidget<'a> {
    pub fn new(view: &'a View<'a>) -> Self {
        Self { view }
    }
}

impl Widget for EditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut RatBuf) {
        // Split: content rows + 1 status row
        let areas = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(area);

        match areas.as_ref() {
            [content_area, status_area] => {
                render_content(self.view, *content_area, buf);
                render_statusline(&self.view.status, *status_area, buf);
            }
            _ => {}
        }
    }
}

// ─── render_frame (called by Tui::render) ───────────────────────────────────

/// Top-level render entry point: draws the editor widget and positions the
/// terminal cursor.
pub fn render_frame(frame: &mut Frame, view: &View) {
    let area = frame.area();
    frame.render_widget(EditorWidget::new(view), area);

    // Position the terminal cursor.
    if let Some(pos) = cursor_screen_pos(view, area) {
        frame.set_cursor_position(pos);
    }
}

// ─── Content area ────────────────────────────────────────────────────────────

fn render_content(view: &View, area: Rect, buf: &mut RatBuf) {
    let gutter_w = gutter_width(view.buffer.len_lines());

    let areas = Layout::horizontal([
        Constraint::Length(gutter_w),
        Constraint::Fill(1),
    ])
    .split(area);

    match areas.as_ref() {
        [gutter_area, text_area] => {
            render_gutter(view, *gutter_area, buf);
            render_text(view, *text_area, buf);
        }
        _ => {}
    }
}

fn render_gutter(view: &View, area: Rect, buf: &mut RatBuf) {
    let cursor_line = view
        .buffer
        .byte_to_line(view.selection.cursor())
        .unwrap_or(0);
    let digits = (gutter_width(view.buffer.len_lines()) as usize).saturating_sub(1);

    for screen_row in 0..(area.height as usize) {
        let line_idx = view.scroll_offset + screen_row;
        let y = area.top() + screen_row as u16;

        if line_idx < view.buffer.len_lines() {
            let num = format!("{:>digits$} ", line_idx + 1);
            let style = if line_idx == cursor_line {
                Style::new().fg(Color::Yellow)
            } else {
                Style::new().fg(Color::DarkGray)
            };
            buf.set_string(area.left(), y, &num, style);
        } else {
            buf.set_string(area.left(), y, "~ ", Style::new().fg(Color::DarkGray));
        }
    }
}

fn render_text(view: &View, area: Rect, buf: &mut RatBuf) {
    let cursor_byte = view.selection.cursor();
    let cursor_line = view.buffer.byte_to_line(cursor_byte).unwrap_or(0);
    let cursor_line_start = view.buffer.line_to_byte(cursor_line).unwrap_or(0);

    for screen_row in 0..(area.height as usize) {
        let line_idx = view.scroll_offset + screen_row;
        let y = area.top() + screen_row as u16;

        let Some(rope_line) = view.buffer.line(line_idx) else {
            continue;
        };

        let line_str = rope_line.to_string();
        let line_text = line_str.trim_end_matches(['\n', '\r']);

        // Cursor-line highlight
        let line_style = if line_idx == cursor_line {
            Style::new().bg(Color::Rgb(40, 40, 50))
        } else {
            Style::default()
        };

        // Fill the line background
        let filler = " ".repeat(area.width as usize);
        buf.set_string(area.left(), y, &filler, line_style);

        // Selection highlight for this line
        let sel_ranges: Vec<(usize, usize)> = view
            .selection
            .ranges()
            .iter()
            .filter_map(|r| {
                if r.is_empty() {
                    return None;
                }
                let line_start = view.buffer.line_to_byte(line_idx).unwrap_or(0);
                let line_end = view
                    .buffer
                    .line_to_byte(line_idx + 1)
                    .unwrap_or(view.buffer.len_bytes());
                let sel_start = r.start().max(line_start);
                let sel_end = r.end().min(line_end);
                if sel_start < sel_end {
                    Some((sel_start - line_start, sel_end - line_start))
                } else {
                    None
                }
            })
            .collect();

        // Write text with selection highlight applied per character.
        // LB-0005 will replace this loop with syntax-highlight spans.
        let mut visual_col: u16 = 0;
        let mut byte_pos: usize = 0;
        for ch in line_text.chars() {
            if visual_col >= area.width {
                break;
            }
            let ch_w = UnicodeWidthStr::width(ch.encode_utf8(&mut [0u8; 4])) as u16;
            let in_sel = sel_ranges
                .iter()
                .any(|(s, e)| byte_pos >= *s && byte_pos < *e);

            let style = if in_sel {
                Style::new().bg(Color::Rgb(60, 90, 120))
            } else {
                line_style
            };

            buf.set_string(area.left() + visual_col, y, &ch.to_string(), style);
            visual_col += ch_w.max(1);
            byte_pos += ch.len_utf8();
        }
    }

    // Draw cursor glyph on top.
    let cursor_col_byte =
        cursor_byte.saturating_sub(cursor_line_start);
    let cursor_visual_col = byte_to_visual_col(
        view.buffer
            .line(cursor_line)
            .map(|s| s.to_string())
            .unwrap_or_default()
            .trim_end_matches(['\n', '\r']),
        cursor_col_byte,
    );
    let cursor_screen_row = cursor_line.saturating_sub(view.scroll_offset) as u16;
    if cursor_line >= view.scroll_offset
        && cursor_screen_row < area.height
        && cursor_visual_col < area.width
    {
        let cell_x = area.left() + cursor_visual_col;
        let cell_y = area.top() + cursor_screen_row;
        let cursor_style = Style::new()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD);
        // Get the char currently at cursor position to show it inverted.
        let ch_str: String = view
            .buffer
            .line(cursor_line)
            .and_then(|s| {
                let raw = s.to_string();
                let text = raw.trim_end_matches(['\n', '\r']);
                text.get(cursor_col_byte..)
                    .and_then(|s| s.chars().next())
                    .map(|c| c.to_string())
            })
            .unwrap_or_else(|| " ".to_string());
        buf.set_string(cell_x, cell_y, &ch_str, cursor_style);
    }
}

// ─── Status line ─────────────────────────────────────────────────────────────

fn render_statusline(status: &StatusLine, area: Rect, buf: &mut RatBuf) {
    let bg = Style::new().bg(Color::Rgb(50, 60, 80)).fg(Color::White);

    // Fill background
    let filler = " ".repeat(area.width as usize);
    buf.set_string(area.left(), area.top(), &filler, bg);

    if let Some(prompt) = &status.prompt {
        // Command or search prompt: show raw prompt string.
        buf.set_string(area.left(), area.top(), prompt, bg);
    } else {
        let left = status.left_text();
        let right = status.right_text();
        let right_x = area
            .right()
            .saturating_sub(right.width() as u16);
        buf.set_string(area.left(), area.top(), &left, bg);
        buf.set_string(right_x, area.top(), &right, bg);
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Width of the gutter: number of digits in the last line number + one space.
fn gutter_width(line_count: usize) -> u16 {
    let digits = if line_count == 0 {
        1u16
    } else {
        line_count.ilog10() as u16 + 1
    };
    digits + 1 // trailing space
}

/// Convert a UTF-8 byte column to a visual (terminal cell) column.
fn byte_to_visual_col(line: &str, byte_col: usize) -> u16 {
    let prefix = line.get(..byte_col).unwrap_or(line);
    UnicodeWidthStr::width(prefix) as u16
}

/// Calculate where the terminal cursor should appear on screen.
fn cursor_screen_pos(view: &View, area: Rect) -> Option<ratatui::layout::Position> {
    let cursor_byte = view.selection.cursor();
    let cursor_line = view.buffer.byte_to_line(cursor_byte)?;

    if cursor_line < view.scroll_offset {
        return None;
    }
    let screen_row = cursor_line - view.scroll_offset;
    let content_height = area.height.saturating_sub(1) as usize; // minus status row
    if screen_row >= content_height {
        return None;
    }

    let line_start = view.buffer.line_to_byte(cursor_line).unwrap_or(0);
    let col_byte = cursor_byte.saturating_sub(line_start);
    let line_str = view
        .buffer
        .line(cursor_line)
        .map(|s| s.to_string())
        .unwrap_or_default();
    let line_text = line_str.trim_end_matches(['\n', '\r']);
    let gutter_w = gutter_width(view.buffer.len_lines());
    let visual_col = byte_to_visual_col(line_text, col_byte);

    Some(ratatui::layout::Position {
        x: area.left() + gutter_w + visual_col,
        y: area.top() + screen_row as u16,
    })
}

// ─── Line (unused helper kept for future highlight spans) ────────────────────

#[allow(dead_code)]
fn styled_line<'a>(text: &'a str, style: Style) -> Line<'a> {
    Line::from(Span::styled(text, style))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use lb_core::{Buffer, Selection};
    use ratatui::{backend::TestBackend, Terminal};

    fn test_view<'a>(
        buf: &'a Buffer,
        sel: &'a Selection,
        status: StatusLine,
    ) -> View<'a> {
        View {
            buffer: buf,
            selection: sel,
            highlights: &[],
            diagnostics: &[],
            status,
            scroll_offset: 0,
        }
    }

    #[test]
    fn render_smoke_does_not_panic() {
        let buf = Buffer::from_str("hello\nworld\n");
        let sel = Selection::point(0);
        let status = StatusLine { filename: Some("test.rs".into()), ..StatusLine::new() };
        let view = test_view(&buf, &sel, status);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_frame(f, &view))
            .unwrap();
    }

    #[test]
    fn render_empty_buffer() {
        let buf = Buffer::new();
        let sel = Selection::point(0);
        let view = test_view(&buf, &sel, StatusLine::new());

        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_frame(f, &view))
            .unwrap();
    }

    #[test]
    fn gutter_width_scales_with_line_count() {
        assert_eq!(gutter_width(9), 2);   // 1 digit + space
        assert_eq!(gutter_width(10), 3);  // 2 digits + space
        assert_eq!(gutter_width(100), 4); // 3 digits + space
    }

    #[test]
    fn byte_to_visual_col_ascii() {
        assert_eq!(byte_to_visual_col("hello", 3), 3);
    }

    #[test]
    fn byte_to_visual_col_multibyte() {
        // "A😀B": A(1) + 😀(4 bytes, 2 cells) + B(1)
        let s = "A\u{1F600}B";
        assert_eq!(byte_to_visual_col(s, 1), 1); // before 😀
        assert_eq!(byte_to_visual_col(s, 5), 3); // before B (2 cells for emoji)
    }
}

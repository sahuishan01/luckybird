//! A centered, keyboard-driven overlay card: a list of rows with one
//! selected, an optional single-line text input, and a footer of key hints.

use ratatui::{
    buffer::Buffer as RatBuf,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Widget},
};

#[derive(Debug, Clone)]
pub struct OverlayRow {
    pub text: String,
    /// Section headers are not selectable and are drawn dim.
    pub selectable: bool,
}

#[derive(Debug, Clone)]
pub struct OverlayModel {
    pub title: String,
    pub rows: Vec<OverlayRow>,
    /// Index into `rows` of the highlighted row.
    pub selected: Option<usize>,
    /// When set, a text field is shown under the rows.
    pub input: Option<String>,
    pub footer: &'static str,
}

const MAX_WIDTH: u16 = 50;

pub fn render(model: &OverlayModel, area: Rect, buf: &mut RatBuf) {
    let width = MAX_WIDTH.min(area.width.saturating_sub(2));
    let extras = 1 + u16::from(model.input.is_some()); // footer (+ input)
    let max_rows = area.height.saturating_sub(2 + extras + 2) as usize;
    let visible = model.rows.len().min(max_rows.max(1));
    let height = visible as u16 + extras + 2;
    if width < 8 || area.height < height {
        return;
    }
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };

    Clear.render(rect, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", model.title))
        .style(Style::new().bg(Color::Rgb(30, 36, 48)).fg(Color::White));
    let inner = block.inner(rect);
    block.render(rect, buf);

    let inner_w = inner.width as usize;
    let selected = model.selected.unwrap_or(0);
    let first = selected.saturating_sub(visible.saturating_sub(1)).min(model.rows.len().saturating_sub(visible));

    for (i, row) in model.rows.iter().enumerate().skip(first).take(visible) {
        let y = inner.y + (i - first) as u16;
        let is_sel = model.selected == Some(i);
        let (prefix, style) = if !row.selectable {
            ("", Style::new().fg(Color::Rgb(130, 140, 160)).add_modifier(Modifier::BOLD))
        } else if is_sel {
            ("> ", Style::new().bg(Color::Rgb(70, 90, 140)).fg(Color::White))
        } else {
            ("  ", Style::new())
        };
        let text: String = format!("{prefix}{}", row.text).chars().take(inner_w).collect();
        buf.set_string(inner.x, y, " ".repeat(inner_w), style);
        buf.set_string(inner.x, y, text, style);
    }

    let mut y = inner.y + visible as u16;
    if let Some(input) = &model.input {
        let shown: String = format!("> {input}_").chars().take(inner_w).collect();
        buf.set_string(inner.x, y, shown, Style::new().fg(Color::Yellow));
        y += 1;
    }
    let footer: String = model.footer.chars().take(inner_w).collect();
    buf.set_string(inner.x, y, footer, Style::new().fg(Color::Rgb(130, 140, 160)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn model(selected: usize) -> OverlayModel {
        OverlayModel {
            title: "Actions".into(),
            rows: vec![
                OverlayRow { text: "File".into(), selectable: false },
                OverlayRow { text: "Save".into(), selectable: true },
                OverlayRow { text: "Save As".into(), selectable: true },
            ],
            selected: Some(selected),
            input: None,
            footer: "j/k move  Enter select  Esc close",
        }
    }

    fn draw(model: &OverlayModel, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(model, f.area(), f.buffer_mut())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_owned()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn draws_title_rows_and_marker() {
        let text = draw(&model(2), 60, 12);
        assert!(text.contains("Actions"));
        assert!(text.contains("> Save As"));
        assert!(text.contains("Esc close"));
    }

    #[test]
    fn tiny_terminal_does_not_panic() {
        let _ = draw(&model(1), 6, 3);
    }

    #[test]
    fn input_line_is_shown() {
        let mut m = model(1);
        m.input = Some("hello.py".into());
        assert!(draw(&m, 60, 12).contains("> hello.py_"));
    }
}

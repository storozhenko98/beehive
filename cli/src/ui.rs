use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Widget},
    Frame,
};

use crate::app::{App, AppMode, Focus, NavItem};

// Catppuccin Mocha
const BASE: Color = Color::Rgb(30, 30, 46);
const MANTLE: Color = Color::Rgb(24, 24, 37);
const CRUST: Color = Color::Rgb(17, 17, 27);
const SURFACE0: Color = Color::Rgb(49, 50, 68);
const SURFACE1: Color = Color::Rgb(69, 71, 90);
const OVERLAY0: Color = Color::Rgb(108, 112, 134);
const TEXT: Color = Color::Rgb(205, 214, 244);
const SUBTEXT0: Color = Color::Rgb(166, 173, 200);
const BLUE: Color = Color::Rgb(137, 180, 250);
const LAVENDER: Color = Color::Rgb(180, 190, 254);
const GREEN: Color = Color::Rgb(166, 227, 161);
const RED: Color = Color::Rgb(243, 139, 168);
const YELLOW: Color = Color::Rgb(249, 226, 175);
const PEACH: Color = Color::Rgb(250, 179, 135);
const MAUVE: Color = Color::Rgb(203, 166, 247);

/// Render the full UI. Returns the terminal pane area (for PTY resize).
pub fn render(frame: &mut Frame, app: &App) -> Rect {
    let bg = Block::default().style(Style::default().bg(CRUST));
    frame.render_widget(bg, frame.area());

    // Main vertical layout: header + content + footer
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(1),   // content
            Constraint::Length(1), // footer
        ])
        .split(frame.area());

    render_header(frame, vert[0]);
    render_footer(frame, app, vert[2]);

    // Content: sidebar | terminal
    let sidebar_width = 28u16.min(frame.area().width / 3).max(20);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(sidebar_width),
            Constraint::Min(1),
        ])
        .split(vert[1]);

    render_sidebar(frame, app, horiz[0]);
    let term_inner = render_terminal_pane(frame, app, horiz[1]);

    // Overlays
    match &app.mode {
        AppMode::Input {
            prompt, value, ..
        } => render_input(frame, prompt, value),
        AppMode::Confirm { message, .. } => render_confirm(frame, message),
        AppMode::Normal => {}
    }

    term_inner
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " beehive",
            Style::default()
                .fg(MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" tui", Style::default().fg(OVERLAY0)),
    ]))
    .style(Style::default().bg(MANTLE));
    frame.render_widget(header, area);
}

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let border_color = if focused { BLUE } else { SURFACE0 };

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(MANTLE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.items.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled("No hives yet", Style::default().fg(SUBTEXT0))),
            Line::from(""),
            Line::from(Span::styled(
                "Press 'a' to add a repo",
                Style::default().fg(OVERLAY0),
            )),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().bg(MANTLE));
        frame.render_widget(msg, inner);
        return;
    }

    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|item| match item {
            NavItem::Hive {
                info,
                expanded,
                comb_count,
            } => {
                let arrow = if *expanded { "▾" } else { "▸" };
                let count_str = if *comb_count > 0 {
                    format!(" ({})", comb_count)
                } else {
                    String::new()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", arrow), Style::default().fg(PEACH)),
                    Span::styled(
                        info.repo_name.clone(),
                        Style::default().fg(PEACH).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(count_str, Style::default().fg(OVERLAY0)),
                ]))
            }
            NavItem::Comb { comb, .. } => {
                let is_active = app
                    .active_comb_id
                    .as_ref()
                    .map(|id| id == &comb.id)
                    .unwrap_or(false);

                let has_terminal = app.terminals.contains_key(&comb.id);

                let (marker, marker_color) = if is_active {
                    ("▶ ", GREEN)
                } else if has_terminal {
                    ("● ", BLUE)
                } else {
                    ("  ", MANTLE)
                };
                let name_color = if is_active { GREEN } else { TEXT };

                ListItem::new(Line::from(vec![
                    Span::styled(format!("   {}", marker), Style::default().fg(marker_color)),
                    Span::styled(comb.name.clone(), Style::default().fg(name_color)),
                    Span::styled(
                        format!(" {}", comb.branch),
                        Style::default().fg(SURFACE1),
                    ),
                ]))
            }
        })
        .collect();

    let mut state = ListState::default();
    if focused {
        state.select(Some(app.selected));
    }

    let list = List::new(items)
        .style(Style::default().bg(MANTLE))
        .highlight_style(
            Style::default()
                .bg(SURFACE0)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, inner, &mut state);
}

/// Render the terminal pane. Returns the inner area (for PTY resize).
fn render_terminal_pane(frame: &mut Frame, app: &App, area: Rect) -> Rect {
    let has_terminal = app.active_terminal().is_some();
    let focused = app.focus == Focus::Terminal;

    if !has_terminal {
        let block = Block::default()
            .style(Style::default().bg(BASE));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Select a comb to open a terminal",
                Style::default().fg(OVERLAY0),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Enter on a comb  |  Ctrl+Space to switch focus",
                Style::default().fg(SURFACE1),
            )),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().bg(BASE));
        frame.render_widget(msg, inner);
        return inner;
    }

    // Terminal with title bar
    let term_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Min(1),   // terminal content
        ])
        .split(area);

    // Title bar
    let comb_name = app.active_comb_name().unwrap_or_default();
    let title_color = if focused { GREEN } else { SUBTEXT0 };
    let title_bg = if focused { SURFACE0 } else { MANTLE };

    let title_bar = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", comb_name),
            Style::default()
                .fg(title_color)
                .bg(title_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if focused { " ● " } else { " ○ " },
            Style::default().fg(if focused { GREEN } else { OVERLAY0 }).bg(title_bg),
        ),
    ]))
    .style(Style::default().bg(title_bg));
    frame.render_widget(title_bar, term_layout[0]);

    // Terminal content
    let block = Block::default()
        .style(Style::default().bg(Color::Reset));
    let inner = block.inner(term_layout[1]);
    frame.render_widget(block, term_layout[1]);

    if let Some(term) = app.active_terminal() {
        term.with_screen(|screen| {
            let widget = Vt100Widget { screen };
            frame.render_widget(widget, inner);
        });
    }

    inner
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(msg) = &app.status_message {
        Line::from(Span::styled(format!(" {}", msg), Style::default().fg(YELLOW)))
    } else if app.focus == Focus::Terminal {
        Line::from(vec![
            Span::styled(" Ctrl+Space", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled(" sidebar", Style::default().fg(OVERLAY0)),
        ])
    } else {
        let mut spans = vec![
            Span::styled(" enter", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled(" open ", Style::default().fg(OVERLAY0)),
            Span::styled("│", Style::default().fg(SURFACE1)),
        ];
        if !app.items.is_empty() {
            spans.extend([
                Span::styled(" n", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
                Span::styled(" new ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
            ]);
        }
        spans.extend([
            Span::styled(" a", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled(" add ", Style::default().fg(OVERLAY0)),
            Span::styled("│", Style::default().fg(SURFACE1)),
            Span::styled(" d", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled(" del ", Style::default().fg(OVERLAY0)),
            Span::styled("│", Style::default().fg(SURFACE1)),
            Span::styled(" q", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled(" quit ", Style::default().fg(OVERLAY0)),
        ]);
        if app.active_terminal().is_some() {
            spans.extend([
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(" C-Spc", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
                Span::styled(" terminal", Style::default().fg(OVERLAY0)),
            ]);
        }
        Line::from(spans)
    };

    let footer = Paragraph::new(content).style(Style::default().bg(MANTLE));
    frame.render_widget(footer, area);
}

// --- vt100 -> ratatui rendering ---

struct Vt100Widget<'a> {
    screen: &'a vt100::Screen,
}

impl<'a> Widget for Vt100Widget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (screen_rows, screen_cols) = self.screen.size();

        for row in 0..area.height.min(screen_rows) {
            for col in 0..area.width.min(screen_cols) {
                if let Some(cell) = self.screen.cell(row, col) {
                    let pos = (area.x + col, area.y + row);
                    if let Some(buf_cell) = buf.cell_mut(pos) {
                        let contents = cell.contents();
                        if contents.is_empty() {
                            buf_cell.set_symbol(" ");
                        } else {
                            buf_cell.set_symbol(&contents);
                        }

                        buf_cell.set_fg(convert_color(cell.fgcolor()));
                        buf_cell.set_bg(convert_color(cell.bgcolor()));

                        let mut modifier = Modifier::empty();
                        if cell.bold() {
                            modifier |= Modifier::BOLD;
                        }
                        if cell.italic() {
                            modifier |= Modifier::ITALIC;
                        }
                        if cell.underline() {
                            modifier |= Modifier::UNDERLINED;
                        }
                        if cell.inverse() {
                            modifier |= Modifier::REVERSED;
                        }
                        buf_cell
                            .set_style(buf_cell.style().add_modifier(modifier));
                    }
                }
            }
        }

        // Cursor
        if !self.screen.hide_cursor() {
            let (crow, ccol) = self.screen.cursor_position();
            if crow < area.height && ccol < area.width {
                if let Some(cursor_cell) = buf.cell_mut((area.x + ccol, area.y + crow)) {
                    cursor_cell.set_style(
                        cursor_cell.style().add_modifier(Modifier::REVERSED),
                    );
                }
            }
        }
    }
}

fn convert_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => match i {
            0 => Color::Black,
            1 => Color::Red,
            2 => Color::Green,
            3 => Color::Yellow,
            4 => Color::Blue,
            5 => Color::Magenta,
            6 => Color::Cyan,
            7 => Color::Gray,
            8 => Color::DarkGray,
            9 => Color::LightRed,
            10 => Color::LightGreen,
            11 => Color::LightYellow,
            12 => Color::LightBlue,
            13 => Color::LightMagenta,
            14 => Color::LightCyan,
            15 => Color::White,
            n => Color::Indexed(n),
        },
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn render_input(frame: &mut Frame, prompt: &str, value: &str) {
    let w = (frame.area().width / 2).max(30).min(frame.area().width.saturating_sub(2));
    let area = centered_rect(w, 3, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {} ", prompt))
        .title_style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(YELLOW))
        .style(Style::default().bg(MANTLE));

    let paragraph = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {}", value), Style::default().fg(TEXT)),
        Span::styled("_", Style::default().fg(YELLOW)),
    ]))
    .block(block);

    frame.render_widget(paragraph, area);
}

fn render_confirm(frame: &mut Frame, message: &str) {
    let w = (frame.area().width / 2).max(30).min(frame.area().width.saturating_sub(2));
    let area = centered_rect(w, 3, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RED))
        .style(Style::default().bg(MANTLE));

    let paragraph = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {} ", message), Style::default().fg(TEXT)),
        Span::styled("y", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("/", Style::default().fg(OVERLAY0)),
        Span::styled("n", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
    ]))
    .block(block);

    frame.render_widget(paragraph, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vert[1]);

    horiz[1]
}

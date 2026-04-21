use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Widget, Wrap},
    Frame,
};

use crate::{
    app::{filter_comb_finder_targets, App, AppMode, Focus, NavItem},
    fuzzy::fuzzy_filter_strings,
};

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
            Constraint::Min(1),    // content
            Constraint::Length(1), // footer
        ])
        .split(frame.area());

    render_header(frame, app, vert[0]);
    render_footer(frame, app, vert[2]);

    // Content: sidebar | terminal
    let sidebar_width = app.sidebar_width.min(frame.area().width / 2).max(20);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
        .split(vert[1]);

    render_sidebar(frame, app, horiz[0]);
    let term_inner = render_terminal_pane(frame, app, horiz[1]);

    // Overlays
    match &app.mode {
        AppMode::Input {
            prompt,
            value,
            cursor,
            ..
        } => render_input(frame, prompt, value, *cursor),
        AppMode::Confirm { message, .. } => render_confirm(frame, message),
        AppMode::Help => render_help(frame),
        AppMode::Settings { preflight } => render_settings(frame, app, preflight),
        AppMode::BranchPicker {
            branches,
            default_branch,
            filter,
            filter_cursor,
            selected,
            comb_name,
            ..
        } => render_branch_picker(
            frame,
            branches,
            default_branch,
            filter,
            *filter_cursor,
            *selected,
            comb_name,
        ),
        AppMode::CombFinder {
            targets,
            filter,
            filter_cursor,
            selected,
        } => render_comb_finder(frame, targets, filter, *filter_cursor, *selected),
        AppMode::Normal | AppMode::MovingComb { .. } | AppMode::DeleteCombSelection { .. } => {}
    }

    term_inner
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let mut left_spans = vec![
        Span::styled(
            " beehive",
            Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(OVERLAY0),
        ),
    ];

    if let Some(ver) = &app.update_available {
        left_spans.push(Span::styled(
            format!("  v{} available — 'u' to update", ver),
            Style::default().fg(GREEN),
        ));
    }

    let left = Paragraph::new(Line::from(left_spans)).style(Style::default().bg(MANTLE));
    frame.render_widget(left, area);

    if matches!(app.mode, AppMode::DeleteCombSelection { .. }) {
        let selected = app.delete_selection_count();
        let label = if selected == 0 {
            " DELETE MODE - pick combs with d ".to_string()
        } else {
            format!(" DELETE MODE - {} selected ", selected)
        };
        let right = Paragraph::new(Line::from(Span::styled(
            label,
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Right)
        .style(Style::default().bg(MANTLE));
        frame.render_widget(right, area);
    } else if matches!(app.mode, AppMode::MovingComb { .. }) {
        let right = Paragraph::new(Line::from(Span::styled(
            " MOVE MODE - refresh paused ",
            Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Right)
        .style(Style::default().bg(MANTLE));
        frame.render_widget(right, area);
    } else if let Some(ref msg) = app.activity_summary() {
        const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let frame_char = SPINNER[(ms / 80) as usize % SPINNER.len()];
        let activity_text = format!("{} {} ", frame_char, msg);
        let right = Paragraph::new(Line::from(Span::styled(
            activity_text,
            Style::default().fg(YELLOW),
        )))
        .alignment(Alignment::Right)
        .style(Style::default().bg(MANTLE));
        frame.render_widget(right, area);
    }
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

    let is_moving = matches!(app.mode, AppMode::MovingComb { .. });
    let delete_mode_hive = app.delete_mode_hive_dir_name();

    let items: Vec<ListItem> = app
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| match item {
            NavItem::Hive {
                info,
                expanded,
                comb_count,
            } => {
                if app.deleting_hive_dir_names.contains(&info.dir_name) {
                    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let frame_char = SPINNER[(ms / 80) as usize % SPINNER.len()];

                    ListItem::new(Line::from(vec![
                        Span::styled(format!(" {} ", frame_char), Style::default().fg(RED)),
                        Span::styled(
                            info.repo_name.clone(),
                            Style::default().fg(RED).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" (deleting)".to_string(), Style::default().fg(SURFACE1)),
                    ]))
                } else {
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
            }
            NavItem::Nest {
                hive_dir_name,
                nest,
                expanded,
                comb_count,
            } => {
                if app.deleting_nest_ids.contains(&nest.id) {
                    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let frame_char = SPINNER[(ms / 80) as usize % SPINNER.len()];

                    ListItem::new(Line::from(vec![
                        Span::styled(format!("   {} ", frame_char), Style::default().fg(RED)),
                        Span::styled(
                            nest.name.clone(),
                            Style::default().fg(RED).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" (deleting)".to_string(), Style::default().fg(SURFACE1)),
                    ]))
                } else if delete_mode_hive == Some(hive_dir_name.as_str()) {
                    let is_marked = app.is_nest_marked_for_delete(&nest.id);
                    let marker = if is_marked { "[x]" } else { "[ ]" };
                    let marker_color = if is_marked { RED } else { OVERLAY0 };
                    let name_color = if is_marked { TEXT } else { YELLOW };
                    let count_str = if *comb_count > 0 {
                        format!(" ({})", comb_count)
                    } else {
                        String::new()
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(format!("   {} ", marker), Style::default().fg(marker_color)),
                        Span::styled(
                            nest.name.clone(),
                            Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(count_str, Style::default().fg(OVERLAY0)),
                    ]))
                } else {
                    let arrow = if *expanded { "▾" } else { "▸" };
                    let count_str = if *comb_count > 0 {
                        format!(" ({})", comb_count)
                    } else {
                        String::new()
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("   {} ", arrow), Style::default().fg(YELLOW)),
                        Span::styled(
                            nest.name.clone(),
                            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(count_str, Style::default().fg(OVERLAY0)),
                    ]))
                }
            }
            NavItem::Comb {
                hive_dir_name,
                comb,
            } => {
                let indent = if comb.nest_id.is_some() {
                    "     "
                } else {
                    "   "
                };
                if comb.cloning {
                    // In-progress comb: animated spinner + dim text
                    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let frame_char = SPINNER[(ms / 80) as usize % SPINNER.len()];

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{}{} ", indent, frame_char),
                            Style::default().fg(YELLOW),
                        ),
                        Span::styled(comb.name.clone(), Style::default().fg(OVERLAY0)),
                        Span::styled(" (in progress)".to_string(), Style::default().fg(SURFACE1)),
                    ]))
                } else if app.deleting_comb_ids.contains(&comb.id) {
                    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let frame_char = SPINNER[(ms / 80) as usize % SPINNER.len()];

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{}{} ", indent, frame_char),
                            Style::default().fg(RED),
                        ),
                        Span::styled(comb.name.clone(), Style::default().fg(RED)),
                        Span::styled(" (deleting)".to_string(), Style::default().fg(SURFACE1)),
                    ]))
                } else if is_moving && i == app.selected {
                    // Comb being moved: mauve highlight with move indicator
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{}↕ ", indent), Style::default().fg(MAUVE)),
                        Span::styled(
                            comb.name.clone(),
                            Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!(" {}", comb.branch), Style::default().fg(SURFACE1)),
                    ]))
                } else if delete_mode_hive == Some(hive_dir_name.as_str()) {
                    let is_marked = app.is_marked_for_delete(&comb.id);
                    let marker = if is_marked { "[x]" } else { "[ ]" };
                    let marker_color = if is_marked { RED } else { OVERLAY0 };
                    let name_color = if is_marked { TEXT } else { SUBTEXT0 };

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{}{} ", indent, marker),
                            Style::default().fg(marker_color),
                        ),
                        Span::styled(comb.name.clone(), Style::default().fg(name_color)),
                        Span::styled(format!(" {}", comb.branch), Style::default().fg(SURFACE1)),
                    ]))
                } else {
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
                        Span::styled(
                            format!("{}{}", indent, marker),
                            Style::default().fg(marker_color),
                        ),
                        Span::styled(comb.name.clone(), Style::default().fg(name_color)),
                        Span::styled(format!(" {}", comb.branch), Style::default().fg(SURFACE1)),
                    ]))
                }
            }
        })
        .collect();

    let mut state = ListState::default();
    if focused {
        state.select(Some(app.selected));
    }

    let list = List::new(items)
        .style(Style::default().bg(MANTLE))
        .highlight_style(Style::default().bg(SURFACE0).add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(list, inner, &mut state);
}

/// Render the terminal pane. Returns the inner area (for PTY resize).
fn render_terminal_pane(frame: &mut Frame, app: &App, area: Rect) -> Rect {
    let has_terminal = app.active_terminal().is_some();
    let focused = app.focus == Focus::Terminal;

    if !has_terminal {
        let block = Block::default().style(Style::default().bg(BASE));
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
            Constraint::Min(1),    // terminal content
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
            Style::default()
                .fg(if focused { GREEN } else { OVERLAY0 })
                .bg(title_bg),
        ),
    ]))
    .style(Style::default().bg(title_bg));
    frame.render_widget(title_bar, term_layout[0]);

    // Terminal content
    let block = Block::default().style(Style::default().bg(Color::Reset));
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
        Line::from(Span::styled(
            format!(" {}", msg),
            Style::default().fg(YELLOW),
        ))
    } else {
        match &app.mode {
            AppMode::Help => Line::from(vec![
                Span::styled(
                    " Esc",
                    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" close", Style::default().fg(OVERLAY0)),
            ]),
            AppMode::Settings { .. } => Line::from(vec![
                Span::styled(
                    " Esc",
                    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" close ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(" R", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
                Span::styled(" reset config", Style::default().fg(OVERLAY0)),
            ]),
            AppMode::BranchPicker { .. } => Line::from(vec![
                Span::styled(
                    " ↑↓",
                    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" navigate ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " enter",
                    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" select ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " type",
                    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to filter ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " Esc",
                    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::default().fg(OVERLAY0)),
            ]),
            AppMode::CombFinder { .. } => Line::from(vec![
                Span::styled(
                    " ↑↓/j/k",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" navigate ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " enter",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" jump ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " type",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to filter ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " Esc",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::default().fg(OVERLAY0)),
            ]),
            AppMode::DeleteCombSelection { .. } => Line::from(vec![
                Span::styled(
                    format!(" {} selected ", app.delete_selection_count()),
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " ↑↓/j/k",
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" navigate ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(" d", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
                Span::styled(" mark ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " enter",
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" delete ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " Esc",
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::default().fg(OVERLAY0)),
            ]),
            AppMode::MovingComb { .. } => Line::from(vec![
                Span::styled(
                    " ↑↓",
                    Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" move ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " m",
                    Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
                ),
                Span::styled("/", Style::default().fg(OVERLAY0)),
                Span::styled(
                    "enter",
                    Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" drop ", Style::default().fg(OVERLAY0)),
                Span::styled("│", Style::default().fg(SURFACE1)),
                Span::styled(
                    " Esc",
                    Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::default().fg(OVERLAY0)),
            ]),
            _ if app.focus == Focus::Terminal => Line::from(vec![
                Span::styled(
                    " Ctrl+Space",
                    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" sidebar", Style::default().fg(OVERLAY0)),
            ]),
            _ => {
                let mut spans = vec![
                    Span::styled(
                        " enter",
                        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" open ", Style::default().fg(OVERLAY0)),
                    Span::styled("│", Style::default().fg(SURFACE1)),
                ];
                if !app.items.is_empty() {
                    spans.extend([
                        Span::styled(
                            " n",
                            Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" comb ", Style::default().fg(OVERLAY0)),
                        Span::styled("│", Style::default().fg(SURFACE1)),
                        Span::styled(
                            " N",
                            Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" nest ", Style::default().fg(OVERLAY0)),
                        Span::styled("│", Style::default().fg(SURFACE1)),
                        Span::styled(
                            " c",
                            Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" copy ", Style::default().fg(OVERLAY0)),
                        Span::styled("│", Style::default().fg(SURFACE1)),
                        Span::styled(
                            " r",
                            Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" rename ", Style::default().fg(OVERLAY0)),
                        Span::styled("│", Style::default().fg(SURFACE1)),
                        Span::styled(
                            " f",
                            Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" find ", Style::default().fg(OVERLAY0)),
                        Span::styled("│", Style::default().fg(SURFACE1)),
                        Span::styled(
                            " m",
                            Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" move ", Style::default().fg(OVERLAY0)),
                        Span::styled("│", Style::default().fg(SURFACE1)),
                    ]);
                }
                spans.extend([
                    Span::styled(
                        " a",
                        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" add ", Style::default().fg(OVERLAY0)),
                    Span::styled("│", Style::default().fg(SURFACE1)),
                    Span::styled(
                        " d",
                        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" del ", Style::default().fg(OVERLAY0)),
                    Span::styled("│", Style::default().fg(SURFACE1)),
                    Span::styled(" D", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
                    Span::styled(" multi-del ", Style::default().fg(OVERLAY0)),
                    Span::styled("│", Style::default().fg(SURFACE1)),
                    Span::styled(
                        " s",
                        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" settings ", Style::default().fg(OVERLAY0)),
                    Span::styled("│", Style::default().fg(SURFACE1)),
                    Span::styled(
                        " </>",
                        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" resize ", Style::default().fg(OVERLAY0)),
                    Span::styled("│", Style::default().fg(SURFACE1)),
                    Span::styled(
                        " ?",
                        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" help ", Style::default().fg(OVERLAY0)),
                    Span::styled("│", Style::default().fg(SURFACE1)),
                    Span::styled(
                        " q",
                        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" quit", Style::default().fg(OVERLAY0)),
                ]);
                if app.active_terminal().is_some() {
                    spans.extend([
                        Span::styled(" │", Style::default().fg(SURFACE1)),
                        Span::styled(
                            " C-Spc",
                            Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" terminal", Style::default().fg(OVERLAY0)),
                    ]);
                }
                if app.update_available.is_some() {
                    spans.extend([
                        Span::styled(" │", Style::default().fg(SURFACE1)),
                        Span::styled(
                            " u",
                            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" update", Style::default().fg(OVERLAY0)),
                    ]);
                }
                Line::from(spans)
            }
        }
    };

    let footer = Paragraph::new(content).style(Style::default().bg(MANTLE));
    frame.render_widget(footer, area);
}

fn render_help(frame: &mut Frame) {
    let area = overlay_rect(60, 25, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help ")
        .title_style(Style::default().fg(MAUVE).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MAUVE))
        .style(Style::default().bg(MANTLE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let key_style = Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(TEXT);
    let header_style = Style::default().fg(PEACH).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(OVERLAY0);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Concepts", header_style)),
        Line::from(Span::styled(
            "  Hive = a GitHub repo you work on",
            dim_style,
        )),
        Line::from(Span::styled(
            "  Nest = an optional group of related combs",
            dim_style,
        )),
        Line::from(Span::styled(
            "  Comb = an isolated git clone / workspace",
            dim_style,
        )),
        Line::from(""),
        Line::from(Span::styled("  Sidebar", header_style)),
        Line::from(vec![
            Span::styled("  enter    ", key_style),
            Span::styled("Open comb / toggle hive", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  j/k      ", key_style),
            Span::styled("Move up/down", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  n        ", key_style),
            Span::styled("New comb (clone + branch)", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  N        ", key_style),
            Span::styled("New nest", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  c        ", key_style),
            Span::styled("Copy comb (duplicate workspace)", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  r        ", key_style),
            Span::styled("Rename selected comb or nest", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  f        ", key_style),
            Span::styled("Find and jump to a comb", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  a        ", key_style),
            Span::styled("Add hive (GitHub repo)", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  d        ", key_style),
            Span::styled("Delete selected comb or hive", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  D        ", key_style),
            Span::styled("Multi-delete mode for combs in a hive", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  </>/H/L  ", key_style),
            Span::styled("Resize sidebar", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  R        ", key_style),
            Span::styled("Refresh sidebar", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  s        ", key_style),
            Span::styled("Settings", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  q        ", key_style),
            Span::styled("Quit", desc_style),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Global", header_style)),
        Line::from(vec![
            Span::styled("  C-Space  ", key_style),
            Span::styled("Toggle sidebar / terminal focus", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C   ", key_style),
            Span::styled("Send interrupt (terminal) / quit (sidebar)", desc_style),
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(MANTLE))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Build spans for a filter input with cursor support.
fn filter_cursor_spans(filter: &str, cursor: usize, cursor_color: Color) -> Vec<Span<'static>> {
    let char_len = filter.chars().count();
    let clamped = cursor.min(char_len);
    let before: String = filter.chars().take(clamped).collect();
    let after: String = filter.chars().skip(clamped).collect();

    let mut spans = vec![
        Span::styled(" / ".to_string(), Style::default().fg(OVERLAY0)),
        Span::styled(before, Style::default().fg(TEXT)),
    ];
    if after.is_empty() {
        spans.push(Span::styled(
            "_".to_string(),
            Style::default().fg(cursor_color),
        ));
    } else {
        let cursor_ch: String = after.chars().take(1).collect();
        let rest: String = after.chars().skip(1).collect();
        spans.push(Span::styled(
            cursor_ch,
            Style::default().fg(MANTLE).bg(cursor_color),
        ));
        if !rest.is_empty() {
            spans.push(Span::styled(rest, Style::default().fg(TEXT)));
        }
    }
    spans
}

fn render_branch_picker(
    frame: &mut Frame,
    branches: &[String],
    default_branch: &str,
    filter: &str,
    filter_cursor: usize,
    selected: usize,
    comb_name: &str,
) {
    let max_h = (frame.area().height - 4).min(20);
    let area = overlay_rect(50, max_h, frame.area());
    frame.render_widget(Clear, area);

    let title = format!(" Branch for '{}' ", comb_name);
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(BLUE).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BLUE))
        .style(Style::default().bg(MANTLE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    // Filter input line
    let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let filter_line = Paragraph::new(Line::from(filter_cursor_spans(filter, filter_cursor, BLUE)))
        .style(Style::default().bg(SURFACE0));
    frame.render_widget(filter_line, filter_area);

    // Branch list
    let list_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);

    let filtered = fuzzy_filter_strings(branches, filter);

    let count_line = Line::from(Span::styled(
        format!(" {} of {} branches", filtered.len(), branches.len()),
        Style::default().fg(OVERLAY0),
    ));
    let count_area = Rect::new(list_area.x, list_area.y, list_area.width, 1);
    frame.render_widget(
        Paragraph::new(count_line).style(Style::default().bg(MANTLE)),
        count_area,
    );

    let items_area = Rect::new(
        list_area.x,
        list_area.y + 1,
        list_area.width,
        list_area.height.saturating_sub(1),
    );

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|name| {
            let is_default = name.as_str() == default_branch;
            let mut spans = vec![Span::styled(
                format!("  {}", name),
                Style::default().fg(TEXT),
            )];
            if is_default {
                spans.push(Span::styled(" (default)", Style::default().fg(GREEN)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(selected));

    let list = List::new(items)
        .style(Style::default().bg(MANTLE))
        .highlight_style(
            Style::default()
                .bg(SURFACE0)
                .fg(BLUE)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, items_area, &mut state);
}

fn render_comb_finder(
    frame: &mut Frame,
    targets: &[crate::app::CombFinderTarget],
    filter: &str,
    filter_cursor: usize,
    selected: usize,
) {
    let max_h = (frame.area().height - 4).min(18);
    let area = overlay_rect(70, max_h, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Jump to Comb ")
        .title_style(Style::default().fg(BLUE).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BLUE))
        .style(Style::default().bg(MANTLE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let filter_line = Paragraph::new(Line::from(filter_cursor_spans(filter, filter_cursor, BLUE)))
        .style(Style::default().bg(SURFACE0));
    frame.render_widget(filter_line, filter_area);

    let list_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
    let filtered = filter_comb_finder_targets(targets, filter);

    let count_area = Rect::new(list_area.x, list_area.y, list_area.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} of {} combs", filtered.len(), targets.len()),
            Style::default().fg(OVERLAY0),
        )))
        .style(Style::default().bg(MANTLE)),
        count_area,
    );

    let items_area = Rect::new(
        list_area.x,
        list_area.y + 1,
        list_area.width,
        list_area.height.saturating_sub(1),
    );

    let items: Vec<ListItem> = if filtered.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  No matching combs",
            Style::default().fg(OVERLAY0),
        )]))]
    } else {
        filtered
            .iter()
            .map(|target| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("  {}", target.comb_name), Style::default().fg(TEXT)),
                    Span::styled(format!(" {}", target.branch), Style::default().fg(SUBTEXT0)),
                    Span::styled(
                        format!("  - {}", target.hive_repo_name),
                        Style::default().fg(OVERLAY0),
                    ),
                ]))
            })
            .collect()
    };

    let mut state = ListState::default();
    state.select((!filtered.is_empty()).then_some(selected));

    let list = List::new(items)
        .style(Style::default().bg(MANTLE))
        .highlight_style(
            Style::default()
                .bg(SURFACE0)
                .fg(BLUE)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, items_area, &mut state);
}

fn render_settings(frame: &mut Frame, app: &App, pf: &crate::config::PreflightResult) {
    let area = overlay_rect(55, 22, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Settings — v{} ", env!("CARGO_PKG_VERSION")))
        .title_style(Style::default().fg(MAUVE).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MAUVE))
        .style(Style::default().bg(MANTLE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let label_style = Style::default().fg(SUBTEXT0);
    let value_style = Style::default().fg(TEXT);
    let header_style = Style::default().fg(PEACH).add_modifier(Modifier::BOLD);

    let ok = Span::styled(
        "OK",
        Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
    );
    let missing = Span::styled(
        "missing",
        Style::default().fg(RED).add_modifier(Modifier::BOLD),
    );
    let not_auth = Span::styled(
        "not authenticated",
        Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
    );

    let git_status = if pf.git.is_some() {
        ok.clone()
    } else {
        missing.clone()
    };
    let gh_status = if pf.gh.is_some() {
        ok.clone()
    } else {
        missing.clone()
    };
    let auth_status = if pf.gh_auth {
        ok
    } else if pf.gh.is_some() {
        not_auth
    } else {
        missing
    };

    let config_path = crate::config::app_config_path()
        .to_string_lossy()
        .to_string();

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Paths", header_style)),
        Line::from(vec![
            Span::styled("  Beehive dir  ", label_style),
            Span::styled(&app.beehive_dir, value_style),
        ]),
        Line::from(vec![
            Span::styled("  Config file  ", label_style),
            Span::styled(config_path, value_style),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Dependencies", header_style)),
        Line::from(vec![
            Span::styled("  git          ", label_style),
            git_status,
        ]),
        Line::from(vec![
            Span::styled("  gh CLI       ", label_style),
            gh_status,
        ]),
        Line::from(vec![
            Span::styled("  gh auth      ", label_style),
            auth_status,
        ]),
        Line::from(""),
        Line::from(Span::styled("  Keyboard", header_style)),
        Line::from(vec![
            Span::styled("  Enhanced keys  ", label_style),
            if app.keyboard_enhanced {
                Span::styled(
                    "active (kitty protocol)",
                    Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    "legacy (Cmd/Shift+Enter limited)",
                    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                )
            },
        ]),
        Line::from(""),
        Line::from(Span::styled("  Sessions", header_style)),
        Line::from(vec![
            Span::styled("  Active terminals  ", label_style),
            Span::styled(format!("{}", app.terminals.len()), value_style),
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(MANTLE))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
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
                        buf_cell.set_style(buf_cell.style().add_modifier(modifier));
                    }
                }
            }
        }

        // Cursor
        if !self.screen.hide_cursor() {
            let (crow, ccol) = self.screen.cursor_position();
            if crow < area.height && ccol < area.width {
                if let Some(cursor_cell) = buf.cell_mut((area.x + ccol, area.y + crow)) {
                    cursor_cell.set_style(cursor_cell.style().add_modifier(Modifier::REVERSED));
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

fn render_input(frame: &mut Frame, prompt: &str, value: &str, cursor: usize) {
    let w = (frame.area().width / 2)
        .max(30)
        .min(frame.area().width.saturating_sub(2));
    let area = centered_rect(w, 3, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {} ", prompt))
        .title_style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(YELLOW))
        .style(Style::default().bg(MANTLE));

    let char_len = value.chars().count();
    let clamped = cursor.min(char_len);
    let before: String = value.chars().take(clamped).collect();
    let after: String = value.chars().skip(clamped).collect();

    let mut spans = vec![Span::styled(
        format!(" {}", before),
        Style::default().fg(TEXT),
    )];
    if after.is_empty() {
        spans.push(Span::styled("_", Style::default().fg(YELLOW)));
    } else {
        let cursor_ch: String = after.chars().take(1).collect();
        let rest: String = after.chars().skip(1).collect();
        spans.push(Span::styled(
            cursor_ch,
            Style::default().fg(MANTLE).bg(YELLOW),
        ));
        if !rest.is_empty() {
            spans.push(Span::styled(rest, Style::default().fg(TEXT)));
        }
    }

    let paragraph = Paragraph::new(Line::from(spans)).block(block);
    frame.render_widget(paragraph, area);
}

fn render_confirm(frame: &mut Frame, message: &str) {
    let w = (frame.area().width / 2)
        .max(30)
        .min(frame.area().width.saturating_sub(2));
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

/// Centered overlay with min/max clamping.
fn overlay_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width.saturating_sub(4));
    let h = height.min(area.height.saturating_sub(2));
    centered_rect(w, h, area)
}

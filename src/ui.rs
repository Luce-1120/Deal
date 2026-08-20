use crate::app::App;
use crate::git::gather;
use crate::model::{Fate, Focus, Mode, Node};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const HUES: [(u8, u8, u8); 10] = [
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
    (137, 180, 250),
];

pub struct Skin {
    pub ink: Color,
    pub soft: Color,
    pub edge: Color,
    pub glow: Color,
    pub plus: Color,
    pub minus: Color,
    pub seat: Color,
    pub tint: [Color; 10],
}

pub fn skin() -> Skin {
    let rich = std::env::var("COLORTERM")
        .map(|v| v.contains("truecolor") || v.contains("24bit"))
        .unwrap_or(false)
        || std::env::var("WT_SESSION").is_ok()
        || std::env::var("TERM_PROGRAM").is_ok();

    if !rich {
        return Skin {
            ink: Color::White,
            soft: Color::Gray,
            edge: Color::DarkGray,
            glow: Color::Cyan,
            plus: Color::Green,
            minus: Color::Red,
            seat: Color::Blue,
            tint: [Color::Cyan; 10],
        };
    }

    let mut tint = [Color::Reset; 10];
    for (i, hue) in HUES.iter().enumerate() {
        tint[i] = Color::Rgb(hue.0, hue.1, hue.2);
    }

    Skin {
        ink: Color::Rgb(205, 214, 244),
        soft: Color::Rgb(147, 153, 178),
        edge: Color::Rgb(69, 71, 90),
        glow: Color::Rgb(137, 180, 250),
        plus: Color::Rgb(166, 227, 161),
        minus: Color::Rgb(243, 139, 168),
        seat: Color::Rgb(45, 52, 74),
        tint,
    }
}

pub fn headline(note: &str) -> &str {
    note.lines().next().unwrap_or("")
}

pub fn panel(app: &App, title: &str, live: bool) -> Block<'static> {
    let edge = match live {
        true => app.skin.glow,
        false => app.skin.edge,
    };
    Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(edge))
        .title(Line::from(vec![
            Span::styled(" ", Style::new()),
            Span::styled(
                title.to_string(),
                Style::new().fg(edge).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::new()),
        ]))
}

pub fn clip(text: &str, room: usize) -> String {
    if text.width() <= room {
        return text.to_string();
    }
    let mark = '…';
    let span = mark.width().unwrap_or(1);
    let target = room.saturating_sub(span);
    let mut width = 0;
    let mut out = String::new();
    for c in text.chars() {
        let cw = c.width().unwrap_or(0);
        if width + cw > target {
            break;
        }
        width += cw;
        out.push(c);
    }
    out.push(mark);
    out
}

pub fn rail(nodes: &[Node], node: usize) -> String {
    let depth = nodes[node].depth;
    if depth == 0 {
        return String::new();
    }
    let stem = match nodes[node].last {
        true => "└ ",
        false => "├ ",
    };
    format!("{}{stem}", "│ ".repeat(depth - 1))
}

pub fn nook(area: Rect, wide: u16, tall: u16) -> Rect {
    let wide = wide.min(area.width.saturating_sub(2));
    let tall = tall.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - wide) / 2,
        y: area.y + (area.height - tall) / 2,
        width: wide,
        height: tall,
    }
}

pub fn paint(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    if area.width < 64 || area.height < 19 {
        let notice = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "window too small",
                Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!(
                    "current {}x{} · minimum 64x19",
                    area.width, area.height
                ),
                Style::new().fg(app.skin.soft),
            )),
        ]));
        frame.render_widget(notice, area);
        return;
    }

    let [top, mid, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(1),
    ])
    .areas(area);

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(mid);

    let [glass, bin] = Layout::vertical([Constraint::Min(6), Constraint::Length(12)]).areas(right);

    crest(frame, app, top);
    tree(frame, app, left);
    diff(frame, app, glass);
    kegs(frame, app, bin);
    bar(frame, app, bottom);

    match app.mode {
        Mode::Help => info(frame, app, area),
        Mode::Ask => prompt(frame, app, area),
        Mode::Test => {
            prompt(frame, app, area);
            test(frame, app, area);
        }
        Mode::Log => {
            prompt(frame, app, area);
            log(frame, app, area);
        }
        Mode::Bail => leave(frame, app, area),
        _ => {}
    }
}

pub fn crest(frame: &mut Frame, app: &App, area: Rect) {
    let dealt = app.nodes.iter().filter(|node| node.mark.is_some()).count();
    let mut spans = vec![
        Span::styled(
            " ❖ ",
            Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "deal",
            Style::new().fg(app.skin.ink).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::new().fg(app.skin.edge)),
        Span::styled(&app.brand, Style::new().fg(app.skin.soft)),
        Span::styled(" ⎇ ", Style::new().fg(app.skin.glow)),
        Span::styled(&app.twig, Style::new().fg(app.skin.ink)),
        Span::styled(" │ ", Style::new().fg(app.skin.edge)),
        Span::styled(
            format!("{dealt}"),
            Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" dealt", Style::new().fg(app.skin.soft)),
        Span::styled(" · ", Style::new().fg(app.skin.edge)),
        Span::styled(
            format!("{} files", app.docs.len()),
            Style::new().fg(app.skin.soft),
        ),
    ];

    if !app.skips.is_empty() {
        spans.push(Span::styled(
            format!(" · {} skipped", app.skips.len()),
            Style::new().fg(app.skin.minus),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub fn tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let room = area.height.saturating_sub(2) as usize;
    if app.cursor < app.scroll {
        app.scroll = app.cursor;
    }
    if room > 0 && app.cursor >= app.scroll + room {
        app.scroll = app.cursor + 1 - room;
    }

    let width = area.width.saturating_sub(8) as usize;
    let mut lines = Vec::new();

    for (row, node) in app.rows.iter().enumerate().skip(app.scroll).take(room) {
        let held = &app.nodes[*node];
        let keg = app.owner(*node);
        let own = held.mark.is_some();
        let selected = row == app.cursor;
        let hue = if keg.is_some() {
            app.skin.glow
        } else {
            app.skin.soft
        };

        let pointer = if selected { "❯" } else { " " };
        let accent = if selected {
            app.skin.glow
        } else {
            app.skin.edge
        };

        let slot = match keg {
            Some(k) => match k {
                9 => "0".to_string(),
                n => (n + 1).to_string(),
            },
            None => "·".to_string(),
        };

        let gutter = match (keg, own) {
            (Some(_), true) => format!("▌{slot} "),
            (Some(_), false) => format!("╎{slot} "),
            (None, _) => " ·  ".to_string(),
        };

        let icon = match held.kids.is_empty() {
            true => "▪",
            false => match held.open {
                true => "▾",
                false => "▸",
            },
        };

        let mut tone = Style::new().fg(match (keg.is_some(), selected) {
            (_, true) => Color::Rgb(255, 255, 255),
            (true, false) => app.skin.ink,
            (false, false) => app.skin.soft,
        });

        if held.depth == 0 || selected {
            tone = tone.add_modifier(Modifier::BOLD);
        }

        let fill = if selected {
            app.skin.seat
        } else {
            Color::Reset
        };

        let inside = app
            .anchor
            .map(|from| row >= from.min(app.cursor) && row <= from.max(app.cursor))
            .unwrap_or(false);

        let mut item = tone.bg(fill);
        if inside && !selected {
            item = item.bg(app.skin.seat).add_modifier(Modifier::ITALIC);
        }

        lines.push(Line::from(vec![
            Span::styled(
                format!("{pointer} "),
                Style::new()
                    .fg(accent)
                    .bg(fill)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                gutter,
                Style::new().fg(hue).bg(fill).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rail(&app.nodes, *node),
                Style::new()
                    .fg(if selected {
                        app.skin.soft
                    } else {
                        app.skin.edge
                    })
                    .bg(fill),
            ),
            Span::styled(
                format!("{icon} "),
                Style::new()
                    .fg(if selected { app.skin.glow } else { hue })
                    .bg(fill),
            ),
            Span::styled(clip(&held.label, width), item),
        ]));
    }

    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(app, "structure", app.focus == Focus::Tree)),
        area,
    );
}

pub fn diff(frame: &mut Frame, app: &mut App, area: Rect) {
    let room = area.height.saturating_sub(2) as usize;
    let lines = shot(app, area.width.saturating_sub(4) as usize);
    if app.view + room > lines.len() {
        app.view = lines.len().saturating_sub(room);
    }

    let crumb = match app.rows.get(app.cursor) {
        Some(node) => trail(app, *node),
        None => "diff preview".to_string(),
    };

    let shown: Vec<Line> = lines.into_iter().skip(app.view).take(room).collect();

    frame.render_widget(
        Paragraph::new(Text::from(shown)).block(panel(
            app,
            &clip(&crumb, area.width as usize - 6),
            app.focus == Focus::Diff,
        )),
        area,
    );
}

pub fn trail(app: &App, node: usize) -> String {
    let mut parts = vec![app.nodes[node].label.clone()];
    let mut step = app.nodes[node].parent;
    while let Some(up) = step {
        parts.push(app.nodes[up].label.clone());
        step = app.nodes[up].parent;
    }
    parts.reverse();
    parts
        .into_iter()
        .map(|part| part.split("  ").next().unwrap_or("").to_string())
        .collect::<Vec<_>>()
        .join(" › ")
}

pub fn shot(app: &App, width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let Some(&node) = app.rows.get(app.cursor) else {
        return out;
    };
    let doc = &app.docs[app.nodes[node].doc];

    if doc.fate == Fate::Gone {
        out.push(Line::from(Span::styled(
            format!("{} deleted", doc.path),
            Style::new().fg(app.skin.minus).add_modifier(Modifier::BOLD),
        )));
        for (i, line) in doc.base.iter().enumerate().take(500) {
            out.push(stripe(app, '-', Some(i + 1), line, width));
        }
        return out;
    }

    if doc.atom {
        out.push(Line::from(Span::styled(
            format!("{} binary ({} bytes)", doc.path, doc.body.len()),
            Style::new().fg(app.skin.soft),
        )));
        return out;
    }

    let mut picks = Vec::new();
    gather(&app.nodes, node, &mut picks);
    picks.sort_unstable();

    if picks.is_empty() {
        out.push(Line::from(Span::styled(
            "no direct changes; expand to view inside",
            Style::new().fg(app.skin.soft),
        )));
        return out;
    }

    for id in picks {
        let Some(cut) = doc.slices.get(id) else {
            continue;
        };
        out.push(Line::from(Span::styled(
            format!(
                "@@ -{},{} +{},{} @@",
                cut.span.start + 1,
                cut.span.len(),
                cut.fresh.start + 1,
                cut.text.len()
            ),
            Style::new().fg(app.skin.glow),
        )));

        let lead = cut.span.start.saturating_sub(2);
        for i in lead..cut.span.start {
            if let Some(line) = doc.base.get(i) {
                out.push(stripe(app, ' ', Some(i + 1), line, width));
            }
        }
        for i in cut.span.clone() {
            if let Some(line) = doc.base.get(i) {
                out.push(stripe(app, '-', Some(i + 1), line, width));
            }
        }
        for (i, line) in cut.text.iter().enumerate() {
            out.push(stripe(app, '+', Some(cut.fresh.start + i + 1), line, width));
        }
        for i in cut.span.end..(cut.span.end + 2) {
            if let Some(line) = doc.base.get(i) {
                out.push(stripe(app, ' ', Some(i + 1), line, width));
            }
        }
        out.push(Line::from(Span::raw("")));
    }

    out
}

pub fn stripe(app: &App, sign: char, no: Option<usize>, text: &str, width: usize) -> Line<'static> {
    let hue = match sign {
        '+' => app.skin.plus,
        '-' => app.skin.minus,
        _ => app.skin.soft,
    };
    Line::from(vec![
        Span::styled(
            match no {
                Some(n) => format!("{n:>5} "),
                None => "      ".to_string(),
            },
            Style::new().fg(app.skin.edge),
        ),
        Span::styled(format!("{sign} "), Style::new().fg(hue)),
        Span::styled(clip(text, width.saturating_sub(8)), Style::new().fg(hue)),
    ])
}

pub fn kegs(frame: &mut Frame, app: &App, area: Rect) {
    let width = area.width.saturating_sub(4) as usize;
    let mut lines = Vec::new();

    for (keg, bin) in app.buckets.iter().enumerate() {
        let hue = app.skin.glow;
        let slot = match keg {
            9 => "0".to_string(),
            n => (n + 1).to_string(),
        };
        let state = match (bin.tally, &bin.flaw) {
            (0, _) => Span::styled("empty", Style::new().fg(app.skin.edge)),
            (_, Some(_)) => Span::styled("error", Style::new().fg(app.skin.minus)),
            (_, None) => Span::styled("sound", Style::new().fg(app.skin.plus)),
        };
        let text = match (bin.note.trim().is_empty(), &bin.flaw) {
            (_, Some(why)) => format!("{why}"),
            (true, None) if bin.tally > 0 => "awaiting message".to_string(),
            (true, None) => "—".to_string(),
            (false, None) => bin.note.trim().to_string(),
        };
        let mut style = Style::new().fg(match bin.tally {
            0 => app.skin.edge,
            _ => app.skin.ink,
        });
        if keg == app.seat {
            style = style.bg(app.skin.seat);
        }

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {slot} "),
                Style::new().fg(hue).add_modifier(Modifier::BOLD),
            ),
            Span::styled("▌", Style::new().fg(hue)),
            Span::styled(
                format!(" {:>3} ", bin.tally),
                Style::new().fg(app.skin.soft),
            ),
            state,
            Span::styled(
                format!("  {}", clip(&text, width.saturating_sub(18))),
                style,
            ),
        ]));
    }

    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(app, "buckets", app.focus == Focus::Kegs)),
        area,
    );
}

pub fn bar(frame: &mut Frame, app: &App, area: Rect) {
    let (chip, hue, hint) = match app.mode {
        Mode::Write => (
            " MESSAGE ",
            app.skin.glow,
            format!("{}▌", app.buckets[app.seat].note),
        ),
        Mode::Find => (" FIND ", app.skin.tint[1], format!("/{}▌", app.query)),
        Mode::Ask => (
            " COMMIT ",
            app.skin.plus,
            "enter commit · v test · e error · esc cancel".to_string(),
        ),
        Mode::Test => (
            " TEST ",
            app.skin.glow,
            format!("command: {}▌", app.cmd),
        ),
        Mode::Log => (
            " LOG ",
            app.skin.minus,
            "j/k scroll · esc/enter close".to_string(),
        ),
        Mode::Bail => (
            " LEAVE ",
            app.skin.minus,
            "y to exit · esc to stay".to_string(),
        ),
        Mode::Help => (
            " HELP ",
            app.skin.glow,
            "press any key to close".to_string(),
        ),
        Mode::Browse => (
            " NORMAL ",
            app.skin.glow,
            format!(
                "tab pane · j/k move · h/l fold · 0-9 deal · u clear · v range · m msg · c commit · ? help   {}",
                app.note
            ),
        ),
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                chip.to_string(),
                Style::new()
                    .fg(Color::Rgb(17, 17, 27))
                    .bg(hue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", clip(&hint, area.width as usize - 12)),
                Style::new().fg(app.skin.soft),
            ),
        ])),
        area,
    );
}

pub fn info(frame: &mut Frame, app: &App, area: Rect) {
    let spot = nook(area, 70, 19);
    let mut lines = vec![
        Line::from(Span::styled(
            "deal groups changes into buckets and creates sequential commits.",
            Style::new().fg(app.skin.ink),
        )),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            "j k   navigate        h l   toggle fold",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::styled(
            "zo zc fold/unfold     zR zM unfold/fold all",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::styled(
            "0-9   assign bucket   u     clear subtree",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::styled(
            "v     visual range    a     jump to root file",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::styled(
            "tab   switch pane     J K   scroll preview",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::styled(
            "m     edit message    < >   reorder buckets",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::styled(
            "/     search label    c     commit          q exit",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            "code blocks stay intact when staged together.",
            Style::new().fg(app.skin.glow),
        )),
        Line::from(Span::styled(
            "working files are not modified; unassigned changes remain in git.",
            Style::new().fg(app.skin.glow),
        )),
    ];

    if !app.skips.is_empty() {
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            "unprocessed paths",
            Style::new().fg(app.skin.minus).add_modifier(Modifier::BOLD),
        )));
        for skip in app.skips.iter().take(3) {
            lines.push(Line::from(Span::styled(
                skip.clone(),
                Style::new().fg(app.skin.soft),
            )));
        }
    }

    frame.render_widget(Clear, spot);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(app, "help", true)),
        spot,
    );
}

pub fn prompt(frame: &mut Frame, app: &App, area: Rect) {
    let spot = nook(area, 76, 17);
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(
            " COMMIT REVIEW ",
            Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    let mut active = 0;
    for (keg, bin) in app.buckets.iter().enumerate() {
        if bin.tally == 0 {
            continue;
        }
        active += 1;
        let slot = match keg {
            9 => "0".to_string(),
            n => (n + 1).to_string(),
        };

        let badge = match bin.verify {
            Some(true) => Span::styled(
                " ● Verified ",
                Style::new().fg(app.skin.plus).add_modifier(Modifier::BOLD),
            ),
            Some(false) => Span::styled(
                " ✖ Failed   ",
                Style::new().fg(app.skin.minus).add_modifier(Modifier::BOLD),
            ),
            None => match &bin.flaw {
                Some(_) => Span::styled(
                    " ▲ Invalid  ",
                    Style::new().fg(app.skin.minus).add_modifier(Modifier::BOLD),
                ),
                None => Span::styled(
                    " ○ Ready    ",
                    Style::new().fg(app.skin.glow),
                ),
            },
        };

        let memo = if bin.note.trim().is_empty() {
            "no message".to_string()
        } else {
            headline(&bin.note).to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {slot} "),
                Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD),
            ),
            Span::styled("▌ ", Style::new().fg(app.skin.glow)),
            Span::styled(
                format!("{:>3} slices  ", bin.tally),
                Style::new().fg(app.skin.soft),
            ),
            Span::styled(
                format!("{:<32}", clip(&memo, 32)),
                Style::new().fg(if bin.note.trim().is_empty() {
                    app.skin.minus
                } else {
                    app.skin.ink
                }),
            ),
            badge,
        ]));
    }

    if active == 0 {
        lines.push(Line::from(Span::styled(
            "  no active buckets",
            Style::new().fg(app.skin.soft),
        )));
    }

    lines.push(Line::raw(""));

    if let Some(why) = app.snag() {
        lines.push(Line::from(vec![
            Span::styled("  Notice: ", Style::new().fg(app.skin.minus).add_modifier(Modifier::BOLD)),
            Span::styled(why, Style::new().fg(app.skin.minus)),
        ]));
    } else if let Some(err) = app.broken {
        let slot = match err {
            9 => "0".to_string(),
            n => (n + 1).to_string(),
        };
        lines.push(Line::from(vec![
            Span::styled("  Failed: ", Style::new().fg(app.skin.minus).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("test failed at bucket {slot} · press "),
                Style::new().fg(app.skin.minus),
            ),
            Span::styled("e", Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD)),
            Span::styled(" to view error", Style::new().fg(app.skin.minus)),
        ]));
    } else if app.buckets.iter().any(|b| b.verify == Some(true)) {
        lines.push(Line::from(vec![
            Span::styled("  Passed: ", Style::new().fg(app.skin.plus).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("all commits passed '{}'", app.cmd.trim()),
                Style::new().fg(app.skin.plus),
            ),
        ]));
    } else if let Some(why) = app.flawed() {
        lines.push(Line::from(vec![
            Span::styled("  Invalid: ", Style::new().fg(app.skin.minus).add_modifier(Modifier::BOLD)),
            Span::styled(why, Style::new().fg(app.skin.minus)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Test: ", Style::new().fg(app.skin.soft).add_modifier(Modifier::BOLD)),
            Span::styled(
                "press v to test commits",
                Style::new().fg(app.skin.soft),
            ),
        ]));
    }

    lines.push(Line::raw(""));

    let mut actions = vec![
        Span::styled(" [Enter]", Style::new().fg(app.skin.plus).add_modifier(Modifier::BOLD)),
        Span::styled(" Commit  ", Style::new().fg(app.skin.ink)),
        Span::styled(" [v]", Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD)),
        Span::styled(" Test  ", Style::new().fg(app.skin.ink)),
    ];
    if app.defect.is_some() {
        actions.push(Span::styled(" [e]", Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD)));
        actions.push(Span::styled(" View Error  ", Style::new().fg(app.skin.ink)));
    }
    actions.push(Span::styled(" [Esc]", Style::new().fg(app.skin.soft).add_modifier(Modifier::BOLD)));
    actions.push(Span::styled(" Cancel ", Style::new().fg(app.skin.ink)));

    lines.push(Line::from(actions));

    frame.render_widget(Clear, spot);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(app, "confirm commit", true)),
        spot,
    );
}

pub fn test(frame: &mut Frame, app: &App, area: Rect) {
    let spot = nook(area, 64, 8);
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "Test command:",
        Style::new().fg(app.skin.ink),
    )));
    lines.push(Line::raw(""));

    let entry = format!("  > {}▌", app.cmd);
    lines.push(Line::from(Span::styled(
        entry,
        Style::new().fg(app.skin.glow).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::styled(" [Enter]", Style::new().fg(app.skin.plus).add_modifier(Modifier::BOLD)),
        Span::styled(" Run   ", Style::new().fg(app.skin.ink)),
        Span::styled(" [Esc]", Style::new().fg(app.skin.soft).add_modifier(Modifier::BOLD)),
        Span::styled(" Back ", Style::new().fg(app.skin.ink)),
    ]));

    frame.render_widget(Clear, spot);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(app, "test command", true)),
        spot,
    );
}

pub fn log(frame: &mut Frame, app: &App, area: Rect) {
    let spot = nook(area, 76, 20);
    let room = spot.height.saturating_sub(4) as usize;
    let width = spot.width.saturating_sub(4) as usize;

    let slot = match app.broken {
        Some(9) => "0".to_string(),
        Some(n) => (n + 1).to_string(),
        None => "?".to_string(),
    };

    let title = format!("error output · bucket {slot}");

    let empty = "no output".to_string();
    let raw = app.defect.as_deref().unwrap_or(&empty);
    let lines_vec: Vec<&str> = raw.lines().collect();

    let scroll = if app.drift + room > lines_vec.len() {
        lines_vec.len().saturating_sub(room)
    } else {
        app.drift
    };

    let mut lines = Vec::new();
    for line in lines_vec.iter().skip(scroll).take(room) {
        lines.push(Line::from(Span::styled(
            clip(line, width),
            Style::new().fg(app.skin.minus),
        )));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "no output",
            Style::new().fg(app.skin.soft),
        )));
    }

    frame.render_widget(Clear, spot);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(app, &title, true)),
        spot,
    );
}

pub fn leave(frame: &mut Frame, app: &App, area: Rect) {
    let spot = nook(area, 56, 6);
    let lines = vec![
        Line::from(Span::styled(
            "uncommitted work in progress.",
            Style::new().fg(app.skin.ink),
        )),
        Line::from(Span::styled(
            "working files will not be changed.",
            Style::new().fg(app.skin.soft),
        )),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            "y to exit · any other key to cancel",
            Style::new().fg(app.skin.minus),
        )),
    ];

    frame.render_widget(Clear, spot);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(app, "abort", true)),
        spot,
    );
}
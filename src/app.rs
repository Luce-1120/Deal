use crate::git::{flow, refresh, weave, wipe};
use crate::model::{Batch, Bucket, Doc, Eol, Fate, Focus, Ledger, Mode, Node, Report};
use crate::syntax::Guard;
use crate::ui::{paint, skin, Skin};
use anyhow::{bail, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use gix::ObjectId;
use ratatui::DefaultTerminal;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub struct App {
    pub docs: Vec<Doc>,
    pub nodes: Vec<Node>,
    pub roots: Vec<usize>,
    pub rows: Vec<usize>,
    pub cursor: usize,
    pub scroll: usize,
    pub view: usize,
    pub seat: usize,
    pub focus: Focus,
    pub mode: Mode,
    pub buckets: Vec<Bucket>,
    pub anchor: Option<usize>,
    pub query: String,
    pub pending: Option<char>,
    pub guard: Guard,
    pub skin: Skin,
    pub brand: String,
    pub twig: String,
    pub whoami: bool,
    pub skips: Vec<String>,
    pub note: String,
    pub fire: bool,
    pub quit: bool,
    pub done: Option<Report>,
    pub work: PathBuf,
    pub cmd: String,
    pub defect: Option<String>,
    pub broken: Option<usize>,
    pub drift: usize,
}

impl App {
    pub fn new(
        batch: Batch,
        brand: String,
        twig: String,
        whoami: bool,
        work: PathBuf,
    ) -> Self {
        let mut app = App {
            docs: batch.docs,
            nodes: batch.nodes,
            roots: batch.roots,
            rows: Vec::new(),
            cursor: 0,
            scroll: 0,
            view: 0,
            seat: 0,
            focus: Focus::Tree,
            mode: Mode::Browse,
            buckets: (0..10)
                .map(|_| Bucket {
                    note: String::new(),
                    flaw: None,
                    tally: 0,
                    verify: None,
                })
                .collect(),
            anchor: None,
            query: String::new(),
            pending: None,
            guard: Guard { pots: BTreeMap::new() },
            skin: skin(),
            brand,
            twig,
            whoami,
            skips: batch.skips,
            note: "press ? for keys".to_string(),
            fire: false,
            quit: false,
            done: None,
            work,
            cmd: String::new(),
            defect: None,
            broken: None,
            drift: 0,
        };
        app.relist();
        app
    }

    pub fn jump(&mut self, next: usize) {
        if self.cursor != next {
            self.cursor = next;
            self.view = 0;
        }
    }

    pub fn relist(&mut self) {
        let mut rows = Vec::new();
        for root in self.roots.clone() {
            flow(&self.nodes, root, &mut rows);
        }
        self.rows = rows;
        if self.cursor >= self.rows.len() {
            self.cursor = self.rows.len().saturating_sub(1);
            self.view = 0;
        }
    }

    pub fn owner(&self, node: usize) -> Option<usize> {
        let mut step = Some(node);
        while let Some(i) = step {
            if let Some(keg) = self.nodes[i].mark {
                return Some(keg);
            }
            step = self.nodes[i].parent;
        }
        None
    }

    pub fn plan(&self) -> Vec<Ledger> {
        let mut out = vec![Ledger::default(); 10];
        for i in 0..self.nodes.len() {
            let Some(keg) = self.owner(i) else { continue };
            let doc = self.nodes[i].doc;
            if self.docs[doc].fate == Fate::Gone {
                out[keg].drops.insert(doc);
                continue;
            }
            let bin = out[keg].keeps.entry(doc).or_default();
            bin.extend(self.nodes[i].slices.iter().copied());
            let mut step = self.nodes[i].parent;
            while let Some(up) = step {
                bin.extend(self.nodes[up].slices.iter().copied());
                step = self.nodes[up].parent;
            }
        }
        out
    }

    pub fn audit(&mut self) {
        let plan = self.plan();
        let Self { docs, guard, buckets, .. } = self;
        let mut carry: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();

        for (keg, ledger) in plan.iter().enumerate() {
            for (doc, picks) in &ledger.keeps {
                carry.entry(*doc).or_default().extend(picks.iter().copied());
            }
            let mut tally = ledger.drops.len();
            for (doc, picks) in &ledger.keeps {
                let paper = &docs[*doc];
                if paper.atom {
                    tally += 1;
                } else {
                    tally += picks.len();
                }
            }
            buckets[keg].tally = tally;
            buckets[keg].flaw = None;
            if ledger.idle() {
                continue;
            }
            for doc in ledger.keeps.keys() {
                let paper = &docs[*doc];
                let (Some(lang), Some(name)) = (paper.lang.clone(), &paper.name) else { continue };
                if paper.atom {
                    continue;
                }
                let Some(picks) = carry.get(doc) else { continue };
                if let Some(spot) = guard.check(lang, name, &weave(paper, picks)) {
                    buckets[keg].flaw = Some(format!("{} · {spot}", paper.path));
                    break;
                }
            }
        }
    }

    pub fn snag(&self) -> Option<String> {
        if !self.whoami {
            return Some(
                "no git identity set; run git config user.name and user.email first"
                    .to_string(),
            );
        }
        let plan = self.plan();
        if plan.iter().all(Ledger::idle) {
            return Some("no changes assigned; press 0-9 to assign to buckets".to_string());
        }
        for (keg, ledger) in plan.iter().enumerate() {
            if !ledger.idle() && self.buckets[keg].note.trim().is_empty() {
                let slot = match keg {
                    9 => "0".to_string(),
                    n => (n + 1).to_string(),
                };
                return Some(format!("bucket {slot} has no message; press m to set"));
            }
        }
        None
    }

    pub fn flawed(&self) -> Option<String> {
        self.buckets
            .iter()
            .enumerate()
            .find_map(|(keg, bin)| {
                let slot = match keg {
                    9 => "0".to_string(),
                    n => (n + 1).to_string(),
                };
                bin.flaw.as_ref().map(|why| format!("bucket {slot} syntax error at {why}"))
            })
    }

    pub fn target(&self) -> Vec<usize> {
        let Some(&here) = self.rows.get(self.cursor) else {
            return Vec::new();
        };
        match self.anchor {
            Some(from) => {
                let lo = from.min(self.cursor);
                let hi = from.max(self.cursor);
                self.rows[lo..=hi.min(self.rows.len() - 1)].to_vec()
            }
            None => vec![here],
        }
    }

    pub fn reset(&mut self) {
        for bucket in &mut self.buckets {
            bucket.verify = None;
        }
        self.defect = None;
        self.broken = None;
        self.drift = 0;
    }

    pub fn deal(&mut self, keg: usize) {
        for node in self.target() {
            push(&mut self.nodes, node);
            for kid in self.nodes[node].kids.clone() {
                wipe(&mut self.nodes, kid);
            }
            self.nodes[node].mark = Some(keg);
            lift(&mut self.nodes, node);
        }
        self.anchor = None;
        self.seat = keg;
        self.audit();
        self.reset();
        let slot = match keg {
            9 => "0".to_string(),
            n => (n + 1).to_string(),
        };
        self.note = format!("assigned to bucket {slot}");
    }

    pub fn clear(&mut self) {
        for node in self.target() {
            push(&mut self.nodes, node);
            wipe(&mut self.nodes, node);
            lift(&mut self.nodes, node);
        }
        self.anchor = None;
        self.audit();
        self.reset();
        self.note = "cleared".to_string();
    }

    pub fn swap(&mut self, other: usize) {
        let here = self.seat;
        self.buckets.swap(here, other);
        for node in &mut self.nodes {
            if node.mark == Some(here) {
                node.mark = Some(other);
            } else if node.mark == Some(other) {
                node.mark = Some(here);
            }
        }
        self.seat = other;
        self.audit();
        self.reset();
    }

    pub fn hunt(&mut self) {
        let needle = self.query.to_lowercase();
        if needle.is_empty() {
            return;
        }
        if let Some(found) = self
            .rows
            .iter()
            .position(|node| self.nodes[*node].label.to_lowercase().contains(&needle))
        {
            self.jump(found);
        }
    }

    pub fn test(&mut self) {
        let plan = self.plan();
        let active: Vec<usize> = plan
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.idle())
            .map(|(k, _)| k)
            .collect();

        if active.is_empty() || self.cmd.trim().is_empty() {
            return;
        }

        let count = active.len();
        let script = self.cmd.trim().to_string();

        struct Backup {
            path: String,
            data: Option<Vec<u8>>,
        }

        struct Rollback {
            dir: PathBuf,
            items: Vec<Backup>,
        }

        impl Drop for Rollback {
            fn drop(&mut self) {
                for item in &self.items {
                    let dest = crate::git::native(&self.dir, &item.path);
                    match &item.data {
                        Some(bytes) => {
                            if let Some(parent) = dest.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(&dest, bytes);
                        }
                        None => {
                            let _ = std::fs::remove_file(&dest);
                        }
                    }
                }
            }
        }

        let _rollback = Rollback {
            dir: self.work.clone(),
            items: self
                .docs
                .iter()
                .map(|doc| Backup {
                    path: doc.path.clone(),
                    data: std::fs::read(crate::git::native(&self.work, &doc.path)).ok(),
                })
                .collect(),
        };

        let apply = |stage: usize,
                     plan: &[Ledger],
                     active: &[usize],
                     docs: &[Doc],
                     dir: &std::path::Path| {
            let mut carry: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
            let mut drops: BTreeSet<usize> = BTreeSet::new();

            for &keg in &active[0..=stage] {
                for (doc, picks) in &plan[keg].keeps {
                    carry.entry(*doc).or_default().extend(picks.iter().copied());
                }
                for doc in &plan[keg].drops {
                    drops.insert(*doc);
                }
            }

            for (idx, doc) in docs.iter().enumerate() {
                let dest = crate::git::native(dir, &doc.path);
                if drops.contains(&idx) {
                    let _ = std::fs::remove_file(&dest);
                    continue;
                }

                if let Some(picks) = carry.get(&idx) {
                    if doc.atom {
                        if let Some(parent) = dest.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(&dest, &doc.body);
                    } else {
                        let text = weave(doc, picks);
                        if let Some(parent) = dest.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(&dest, text.as_bytes());
                    }
                } else {
                    match doc.fate {
                        Fate::Born => {
                            let _ = std::fs::remove_file(&dest);
                        }
                        Fate::Edit | Fate::Gone => {
                            let glue = match doc.eol {
                                Eol::Lf => "\n",
                                Eol::Crlf => "\r\n",
                            };
                            let mut text = doc.base.join(glue);
                            if doc.tail && !text.is_empty() {
                                text.push_str(glue);
                            }
                            if let Some(parent) = dest.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(&dest, text.as_bytes());
                        }
                    }
                }
            }
        };

        let execute = |run: &str, dir: &std::path::Path| -> (bool, String) {
            #[cfg(windows)]
            let mut job = {
                let mut cmd = std::process::Command::new("cmd");
                cmd.args(["/C", run]);
                cmd
            };

            #[cfg(not(windows))]
            let mut job = {
                let mut cmd = std::process::Command::new("sh");
                cmd.args(["-c", run]);
                cmd
            };

            job.current_dir(dir);

            match job.output() {
                Ok(output) => {
                    let ok = output.status.success();
                    let mut msg = String::new();
                    if !output.stdout.is_empty() {
                        msg.push_str(&String::from_utf8_lossy(&output.stdout));
                    }
                    if !output.stderr.is_empty() {
                        if !msg.is_empty() && !msg.ends_with('\n') {
                            msg.push('\n');
                        }
                        msg.push_str(&String::from_utf8_lossy(&output.stderr));
                    }
                    if msg.trim().is_empty() {
                        msg = if ok {
                            "command succeeded".to_string()
                        } else {
                            format!("command exited with status {:?}", output.status.code())
                        };
                    }
                    (ok, msg)
                }
                Err(err) => (false, format!("failed to run command: {err}")),
            }
        };

        apply(count - 1, &plan, &active, &self.docs, &self.work);
        let (pass, output) = execute(&script, &self.work);

        if pass {
            for &keg in &active {
                self.buckets[keg].verify = Some(true);
            }
            self.defect = None;
            self.broken = None;
        } else {
            let mut stop = count - 1;
            let mut cause = output;
            let mut low = 0;
            let mut high = count - 1;

            while low < high {
                let mid = low + (high - low) / 2;
                apply(mid, &plan, &active, &self.docs, &self.work);
                let (ok, res) = execute(&script, &self.work);
                if ok {
                    low = mid + 1;
                } else {
                    stop = mid;
                    cause = res;
                    high = mid;
                }
            }

            for (i, &keg) in active.iter().enumerate() {
                if i < stop {
                    self.buckets[keg].verify = Some(true);
                } else {
                    self.buckets[keg].verify = Some(false);
                }
            }
            self.defect = Some(cause);
            self.broken = Some(active[stop]);
        }
    }

    pub fn key(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Write => match key.code {
                KeyCode::Esc => self.mode = Mode::Browse,
                KeyCode::Enter => {
                    self.mode = Mode::Browse;
                    self.audit();
                }
                KeyCode::Backspace => {
                    self.buckets[self.seat].note.pop();
                }
                KeyCode::Char(c) => self.buckets[self.seat].note.push(c),
                _ => {}
            },
            Mode::Find => match key.code {
                KeyCode::Esc => {
                    self.query.clear();
                    self.mode = Mode::Browse;
                }
                KeyCode::Enter => self.mode = Mode::Browse,
                KeyCode::Backspace => {
                    self.query.pop();
                    self.hunt();
                }
                KeyCode::Char(c) => {
                    self.query.push(c);
                    self.hunt();
                }
                _ => {}
            },
            Mode::Ask => match key.code {
                KeyCode::Enter => match self.snag() {
                    None => self.fire = true,
                    Some(why) => self.note = why,
                },
                KeyCode::Char('v') => {
                    self.mode = Mode::Test;
                }
                KeyCode::Char('e') => {
                    if self.defect.is_some() {
                        self.mode = Mode::Log;
                        self.drift = 0;
                    }
                }
                KeyCode::Esc => self.mode = Mode::Browse,
                _ => {}
            },
            Mode::Test => match key.code {
                KeyCode::Esc => self.mode = Mode::Ask,
                KeyCode::Enter => {
                    self.test();
                    self.mode = Mode::Ask;
                }
                KeyCode::Backspace => {
                    self.cmd.pop();
                }
                KeyCode::Char(c) => {
                    self.cmd.push(c);
                }
                _ => {}
            },
            Mode::Log => match key.code {
                KeyCode::Esc
                | KeyCode::Enter
                | KeyCode::Char('e')
                | KeyCode::Char('q') => {
                    self.mode = Mode::Ask;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.drift += 1;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.drift = self.drift.saturating_sub(1);
                }
                KeyCode::Char('g') => {
                    self.drift = 0;
                }
                KeyCode::Char('G') => {
                    self.drift = usize::MAX / 2;
                }
                _ => {}
            },
            Mode::Bail => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.quit = true,
                _ => self.mode = Mode::Browse,
            },
            Mode::Help => self.mode = Mode::Browse,
            Mode::Browse => self.browse(key),
        }
    }

    pub fn browse(&mut self, key: KeyEvent) {
        if let Some('z') = self.pending {
            self.pending = None;
            match key.code {
                KeyCode::Char('o') => self.unfold(true),
                KeyCode::Char('c') => self.unfold(false),
                KeyCode::Char('R') => self.sweep(true),
                KeyCode::Char('M') => self.sweep(false),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('z') => self.pending = Some('z'),
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Tree => Focus::Diff,
                    Focus::Diff => Focus::Kegs,
                    Focus::Kegs => Focus::Tree,
                }
            }
            KeyCode::Char('j') | KeyCode::Down => match self.focus {
                Focus::Tree => self.jump((self.cursor + 1).min(self.rows.len().saturating_sub(1))),
                Focus::Diff => self.view += 1,
                Focus::Kegs => self.seat = (self.seat + 1).min(9),
            },
            KeyCode::Char('k') | KeyCode::Up => match self.focus {
                Focus::Tree => self.jump(self.cursor.saturating_sub(1)),
                Focus::Diff => self.view = self.view.saturating_sub(1),
                Focus::Kegs => self.seat = self.seat.saturating_sub(1),
            },
            KeyCode::Char('g') => match self.focus {
                Focus::Tree => self.jump(0),
                Focus::Diff => self.view = 0,
                Focus::Kegs => self.seat = 0,
            },
            KeyCode::Char('G') => match self.focus {
                Focus::Tree => self.jump(self.rows.len().saturating_sub(1)),
                Focus::Diff => self.view = usize::MAX / 2,
                Focus::Kegs => self.seat = 9,
            },
            KeyCode::Char('J') => self.view += 1,
            KeyCode::Char('K') => self.view = self.view.saturating_sub(1),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => match self.focus {
                Focus::Tree => self.unfold(true),
                _ => {}
            },
            KeyCode::Char('h') | KeyCode::Left => match self.focus {
                Focus::Tree => self.unfold(false),
                _ => {}
            },
            KeyCode::Char('v') => {
                self.anchor = match self.anchor {
                    Some(_) => None,
                    None => Some(self.cursor),
                };
                self.note = match self.anchor {
                    Some(_) => "range mode active".to_string(),
                    None => "range cleared".to_string(),
                };
            }
            KeyCode::Char('a') => {
                if let Some(&here) = self.rows.get(self.cursor) {
                    let mut step = here;
                    while let Some(up) = self.nodes[step].parent {
                        step = up;
                    }
                    if let Some(pos) = self.rows.iter().position(|node| *node == step) {
                        self.jump(pos);
                    }
                    self.anchor = None;
                    self.note = "file root selected".to_string();
                }
            }
            KeyCode::Char('u') => self.clear(),
            KeyCode::Char('m') => self.mode = Mode::Write,
            KeyCode::Char('>') => self.swap((self.seat + 1).min(9)),
            KeyCode::Char('<') => self.swap(self.seat.saturating_sub(1)),
            KeyCode::Char('/') => {
                self.query.clear();
                self.mode = Mode::Find;
            }
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('c') => self.mode = Mode::Ask,
            KeyCode::Char('q') | KeyCode::Esc => {
                self.mode = match self.nodes.iter().any(|node| node.mark.is_some()) {
                    true => Mode::Bail,
                    false => {
                        self.quit = true;
                        Mode::Browse
                    }
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let keg = match c {
                    '1'..='9' => c as usize - '1' as usize,
                    '0' => 9,
                    _ => return,
                };
                match self.focus {
                    Focus::Tree | Focus::Diff => self.deal(keg),
                    Focus::Kegs => self.seat = keg,
                }
            }
            _ => {}
        }
    }

    pub fn unfold(&mut self, open: bool) {
        let Some(&here) = self.rows.get(self.cursor) else { return };
        match (open, self.nodes[here].kids.is_empty()) {
            (true, false) => self.nodes[here].open = true,
            (false, _) if self.nodes[here].open && !self.nodes[here].kids.is_empty() => {
                self.nodes[here].open = false
            }
            (false, _) => {
                if let Some(up) = self.nodes[here].parent {
                    self.nodes[up].open = false;
                    if let Some(pos) = self.rows.iter().position(|node| *node == up) {
                        self.jump(pos);
                    }
                }
            }
            _ => {}
        }
        self.relist();
    }

    pub fn sweep(&mut self, open: bool) {
        for node in &mut self.nodes {
            node.open = open || node.depth == 0;
        }
        self.relist();
    }
}

fn push(nodes: &mut [Node], node: usize) {
    let mut path = Vec::new();
    let mut step = nodes[node].parent;
    while let Some(up) = step {
        path.push(up);
        step = nodes[up].parent;
    }
    path.reverse();

    for up in path {
        if let Some(keg) = nodes[up].mark.take() {
            for kid in nodes[up].kids.clone() {
                nodes[kid].mark = Some(keg);
            }
        }
    }
}

fn lift(nodes: &mut [Node], node: usize) {
    let mut step = nodes[node].parent;
    while let Some(up) = step {
        if nodes[up].kids.is_empty() {
            step = nodes[up].parent;
            continue;
        }

        let first = nodes[nodes[up].kids[0]].mark;
        let uniform = first.is_some() && nodes[up].kids.iter().all(|&kid| nodes[kid].mark == first);

        if uniform {
            nodes[up].mark = first;
            for kid in nodes[up].kids.clone() {
                nodes[kid].mark = None;
            }
        } else {
            nodes[up].mark = None;
        }

        step = nodes[up].parent;
    }
}

pub fn drive(terminal: &mut DefaultTerminal, app: &mut App, repo: &gix::Repository) -> Result<()> {
    while !app.quit {
        terminal
            .draw(|frame| paint(frame, app))
            .context("terminal render failed")?;

        match event::read().context("terminal input read failed")? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('C'))
                {
                    app.quit = true;
                    break;
                }
                app.key(key);
            }
            _ => {}
        }

        if app.fire {
            app.fire = false;
            app.done = Some(commit(repo, app).context(
                "commit failed; inspect git log",
            )?);
            app.quit = true;
        }
    }
    Ok(())
}

pub fn commit(repo: &gix::Repository, app: &App) -> Result<Report> {
    let base = repo
        .head_tree_id_or_empty()
        .context("failed to resolve HEAD tree")?
        .detach();

    let mut editor = repo
        .edit_tree(base)
        .context("failed to open tree editor")?;

    let mut parent: Option<ObjectId> = repo.head_id().ok().map(|id| id.detach());
    let mut carry: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut commits = Vec::new();
    let mut last = base;

    for (keg, ledger) in app.plan().into_iter().enumerate() {
        if ledger.idle() {
            continue;
        }

        let slot = match keg {
            9 => "0".to_string(),
            n => (n + 1).to_string(),
        };

        let note = app.buckets[keg].note.trim().to_string();
        if note.is_empty() {
            bail!("bucket {slot} has no message");
        }

        for (doc, picks) in &ledger.keeps {
            carry.entry(*doc).or_default().extend(picks.iter().copied());
        }

        for doc in ledger.drops.iter() {
            let paper = &app.docs[*doc];
            editor
                .remove(&paper.path)
                .with_context(|| format!("failed to remove '{}'", paper.path))?;
        }

        for doc in ledger.keeps.keys() {
            let paper = &app.docs[*doc];
            let blob = match paper.atom {
                true => repo.write_blob(&paper.body),
                false => {
                    let picks = carry.get(doc).cloned().unwrap_or_default();
                    repo.write_blob(weave(paper, &picks).as_bytes())
                }
            }
            .with_context(|| format!("failed to write blob for '{}'", paper.path))?;

            editor
                .upsert(&paper.path, paper.mode, blob.detach())
                .with_context(|| format!("failed to stage '{}'", paper.path))?;
        }

        let tree = editor
            .write()
            .with_context(|| format!("failed to create tree for bucket {slot}"))?
            .detach();

        let id = repo
            .commit("HEAD", &note, tree, parent.into_iter().collect::<Vec<_>>())
            .with_context(|| format!("failed to create commit for bucket {slot}"))?
            .detach();

        parent = Some(id);
        last = tree;
        commits.push((id, note));
    }

    let gripe = refresh(repo, last);
    Ok(Report { commits, gripe })
}
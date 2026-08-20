use crate::git::tag;
use crate::model::{Doc, Node};
use std::collections::BTreeMap;
use std::ops::Range;
use tree_sitter::{Language, Node as Twig, Parser as Sitter, Tree, TreeCursor};

pub fn tongue(path: &str) -> (Option<Language>, Option<String>) {
    let ext = path.rsplit('.').next().unwrap_or("");
    let name = tree_sitter_language_pack::detect_language_from_extension(ext)
        .or_else(|| if ext.is_empty() { None } else { Some(ext) });

    let Some(tag) = name else {
        return (None, None);
    };

    match tree_sitter_language_pack::get_language(tag) {
        Ok(lang) => (Some(lang), Some(tag.to_string())),
        Err(_) => (None, None),
    }
}

pub fn parse(lang: Language, src: &str) -> Option<Tree> {
    let mut sitter = Sitter::new();
    sitter.set_language(&lang).ok()?;
    sitter.parse(src, None)
}

pub struct Guard {
    pub pots: BTreeMap<String, Sitter>,
}

impl Guard {
    pub fn check(&mut self, lang: Language, name: &str, text: &str) -> Option<String> {
        if !self.pots.contains_key(name) {
            let mut sitter = Sitter::new();
            if sitter.set_language(&lang).is_err() {
                return None;
            }
            self.pots.insert(name.to_string(), sitter);
        }

        let sitter = self.pots.get_mut(name)?;
        let tree = sitter.parse(text, None)?;
        let root = tree.root_node();
        if !root.has_error() {
            return None;
        }

        let mut walker = root.walk();
        Some(fault(&mut walker).unwrap_or_else(|| "syntax error".to_string()))
    }
}

pub fn fault(walker: &mut TreeCursor) -> Option<String> {
    let node = walker.node();
    if node.is_error() || node.is_missing() {
        let spot = node.start_position();
        return Some(format!("line {}, column {}", spot.row + 1, spot.column + 1));
    }
    if walker.goto_first_child() {
        loop {
            if let Some(found) = fault(walker) {
                return Some(found);
            }
            if !walker.goto_next_sibling() {
                break;
            }
        }
        walker.goto_parent();
    }
    None
}

pub fn frame(kind: &str) -> bool {
    kind.contains("function")
        || kind.contains("method")
        || kind.contains("class")
        || kind.contains("struct")
        || kind.contains("interface")
        || kind.contains("trait")
        || kind.contains("impl")
        || kind.contains("enum")
        || kind.contains("module")
        || kind.contains("namespace")
        || kind.contains("package")
        || kind.contains("block")
        || kind.contains("declaration")
        || kind.contains("definition")
        || kind.contains("statement")
        || kind.contains("item")
}

pub fn branches<'a>(twig: Twig<'a>, out: &mut Vec<Twig<'a>>) {
    let mut walker = twig.walk();
    for kid in twig.named_children(&mut walker) {
        match frame(kid.kind()) {
            true => out.push(kid),
            false => branches(kid, out),
        }
    }
}

pub fn holds(twig: Twig, span: &Range<usize>) -> bool {
    let lo = twig.start_position().row;
    let hi = twig.end_position().row;
    match span.is_empty() {
        true => span.start >= lo && span.start <= hi,
        false => span.start >= lo && span.end <= hi + 1,
    }
}

pub fn dub(twig: Twig, src: &str) -> String {
    let kind = twig.kind().replace('_', " ");
    match twig
        .child_by_field_name("name")
        .and_then(|name| name.utf8_text(src.as_bytes()).ok())
    {
        Some(name) => format!("{kind} {name}"),
        None => format!("{kind} · line {}", twig.start_position().row + 1),
    }
}

pub fn sprout<'a>(
    nodes: &mut Vec<Node>,
    doc: usize,
    parent: usize,
    depth: usize,
    twig: Twig<'a>,
    src: &str,
    paper: &Doc,
    spots: Vec<(usize, Range<usize>)>,
) {
    let mut nest = Vec::new();
    if depth < 5 {
        branches(twig, &mut nest);
    }

    let mut sorted: Vec<(usize, Vec<(usize, Range<usize>)>)> = Vec::new();
    let mut loose: Vec<(usize, Range<usize>)> = Vec::new();

    for spot in spots {
        match nest.iter().position(|kid| holds(*kid, &spot.1)) {
            Some(slot) => match sorted.iter_mut().find(|held| held.0 == slot) {
                Some(held) => held.1.push(spot),
                None => sorted.push((slot, vec![spot])),
            },
            None => loose.push(spot),
        }
    }

    let mut plan: Vec<(usize, Option<usize>, Vec<(usize, Range<usize>)>)> = Vec::new();
    for spot in loose {
        plan.push((spot.1.start, None, vec![spot]));
    }
    for held in sorted {
        plan.push((nest[held.0].start_position().row, Some(held.0), held.1));
    }
    plan.sort_by_key(|step| step.0);

    for step in plan {
        match step.1 {
            None => {
                let ids: Vec<usize> = step.2.iter().map(|spot| spot.0).collect();
                let label = ids
                    .first()
                    .map(|id| tag(paper, *id))
                    .unwrap_or_else(|| "slice".to_string());
                crate::git::leaf(nodes, doc, parent, depth, label, ids);
            }
            Some(slot) => {
                let kid = nest[slot];
                let index = nodes.len();
                nodes.push(Node {
                    doc,
                    parent: Some(parent),
                    kids: Vec::new(),
                    depth,
                    last: false,
                    label: dub(kid, src),
                    slices: Vec::new(),
                    open: true,
                    mark: None,
                });
                nodes[parent].kids.push(index);
                sprout(nodes, doc, index, depth + 1, kid, src, paper, step.2);
            }
        }
    }
}
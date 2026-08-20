use crate::model::{Batch, CAP, Doc, Eol, Fate, Node, Slice};
use crate::syntax::{parse, sprout, tongue};
use anyhow::{Context, Result};
use gix::ObjectId;
use gix::bstr::{BString, ByteSlice};
use gix::object::tree::EntryKind;
use similar::{Algorithm, DiffTag, capture_diff_slices};
use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub fn harvest(repo: &gix::Repository, work: &Path) -> Result<Batch> {
    let tip = repo
        .head_tree_id_or_empty()
        .context("failed to resolve HEAD tree")?
        .detach();

    let tree = repo
        .find_tree(tip)
        .context("HEAD tree missing from database")?;

    let walk = repo
        .status(gix::progress::Discard)
        .context("failed to read git status")?
        .index_worktree_submodules(None)
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(Vec::<BString>::new())
        .context("failed to read git status")?;

    let mut paths: BTreeSet<String> = BTreeSet::new();
    for item in walk {
        let item = item.context("failed while reading git status")?;
        paths.insert(item.location().to_str_lossy().into_owned());
    }

    let mut batch = Batch {
        docs: Vec::new(),
        nodes: Vec::new(),
        roots: Vec::new(),
        skips: Vec::new(),
    };

    for path in paths {
        let full = native(work, &path);

        let entry = tree
            .lookup_entry_by_path(Path::new(&path))
            .with_context(|| format!("failed to find '{path}' in HEAD"))?;

        let old = match &entry {
            Some(found) => Some(
                found
                    .object()
                    .with_context(|| format!("missing blob for '{path}'"))?
                    .data
                    .clone(),
            ),
            None => None,
        };

        let link = std::fs::symlink_metadata(&full)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        if link {
            batch.skips.push(format!("'{path}' (symlink)"));
            continue;
        }

        let big = std::fs::metadata(&full)
            .map(|meta| meta.len() > CAP)
            .unwrap_or(false);
        let new = match std::fs::read(&full) {
            Ok(bytes) => Some(bytes),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
            Err(why) if why.kind() == std::io::ErrorKind::PermissionDenied => {
                batch
                    .skips
                    .push(format!("'{path}' (permission denied: {why})"));
                continue;
            }
            Err(_) => None,
        };

        let (fate, old, new) = match (old, new) {
            (Some(a), Some(b)) if a == b => continue,
            (Some(a), Some(b)) => (Fate::Edit, a, b),
            (Some(a), None) => (Fate::Gone, a, Vec::new()),
            (None, Some(b)) => (Fate::Born, Vec::new(), b),
            (None, None) => continue,
        };

        let prior = entry.as_ref().map(|found| found.mode().kind());
        let mode = stamp(&full, prior);

        let doc = match fate {
            Fate::Gone => Doc {
                path: path.clone(),
                fate,
                mode,
                eol: Eol::Lf,
                tail: false,
                atom: false,
                base: text(&old).map(shred).unwrap_or_default(),
                body: Vec::new(),
                slices: Vec::new(),
                lang: None,
                name: None,
            },
            _ => {
                let pair = (text(&old), text(&new));
                match pair {
                    (Some(before), Some(after)) if !big => {
                        build(path.clone(), fate, mode, before, after)
                    }
                    _ => Doc {
                        path: path.clone(),
                        fate,
                        mode,
                        eol: Eol::Lf,
                        tail: false,
                        atom: true,
                        base: Vec::new(),
                        body: new.clone(),
                        slices: Vec::new(),
                        lang: None,
                        name: None,
                    },
                }
            }
        };

        let index = batch.docs.len();
        let after = match doc.atom || doc.fate == Fate::Gone {
            true => None,
            false => text(&new),
        };

        batch.docs.push(doc);
        plant(&mut batch, index, after.as_deref());
    }

    for i in 0..batch.nodes.len() {
        if let Some(last) = batch.nodes[i].kids.last().copied() {
            batch.nodes[last].last = true;
        }
    }

    let roomy = batch.nodes.len() <= 60;
    for node in &mut batch.nodes {
        node.open = roomy || node.depth == 0;
    }

    Ok(batch)
}

pub fn native(work: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(work.to_path_buf(), |acc, part| acc.join(part))
}

pub fn text(raw: &[u8]) -> Option<String> {
    if raw.contains(&0) {
        return None;
    }
    String::from_utf8(raw.to_vec()).ok()
}

pub fn shred(body: String) -> Vec<String> {
    body.lines().map(str::to_string).collect()
}

pub fn stamp(full: &Path, prior: Option<EntryKind>) -> EntryKind {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(full) {
            return match meta.permissions().mode() & 0o111 != 0 {
                true => EntryKind::BlobExecutable,
                false => EntryKind::Blob,
            };
        }
    }
    #[cfg(not(unix))]
    let _ = full;
    prior.unwrap_or(EntryKind::Blob)
}

pub fn build(path: String, fate: Fate, mode: EntryKind, before: String, after: String) -> Doc {
    let eol = match before.contains("\r\n") || (before.is_empty() && after.contains("\r\n")) {
        true => Eol::Crlf,
        false => Eol::Lf,
    };
    let tail = match fate {
        Fate::Gone => false,
        _ => after.ends_with('\n'),
    };
    let change = before.ends_with('\n') != after.ends_with('\n');

    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let ops = capture_diff_slices(Algorithm::Myers, &old, &new);

    let mut slices = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        if ops[i].tag() == DiffTag::Equal {
            i += 1;
            continue;
        }
        let mut span = ops[i].old_range();
        let mut fresh = ops[i].new_range();
        let mut j = i + 1;
        while j < ops.len() && ops[j].tag() != DiffTag::Equal {
            span.end = ops[j].old_range().end;
            fresh.end = ops[j].new_range().end;
            j += 1;
        }
        slices.push(Slice {
            id: slices.len(),
            span,
            fresh: fresh.clone(),
            text: new[fresh].iter().map(|line| line.to_string()).collect(),
        });
        i = j;
    }

    if slices.is_empty() && change && !new.is_empty() {
        let last = new.len().saturating_sub(1);
        slices.push(Slice {
            id: 0,
            span: last..last + 1,
            fresh: last..last + 1,
            text: vec![new[last].to_string()],
        });
    }

    let (lang, name) = tongue(&path);

    Doc {
        path: path.clone(),
        fate,
        mode,
        eol,
        tail,
        atom: false,
        base: old.iter().map(|line| line.to_string()).collect(),
        body: Vec::new(),
        slices,
        lang,
        name,
    }
}

pub fn plant(batch: &mut Batch, doc: usize, after: Option<&str>) {
    let label = crown(&batch.docs[doc]);
    let root = batch.nodes.len();
    batch.nodes.push(Node {
        doc,
        parent: None,
        kids: Vec::new(),
        depth: 0,
        last: false,
        label,
        slices: Vec::new(),
        open: true,
        mark: None,
    });
    batch.roots.push(root);

    if batch.docs[doc].fate == Fate::Gone || batch.docs[doc].atom {
        return;
    }

    let spots: Vec<(usize, Range<usize>)> = batch.docs[doc]
        .slices
        .iter()
        .map(|cut| (cut.id, cut.fresh.clone()))
        .collect();

    let tree = match (batch.docs[doc].lang.clone(), after) {
        (Some(lang), Some(src)) => parse(lang, src).map(|tree| (src, tree)),
        _ => None,
    };

    match tree {
        Some((src, tree)) => {
            let paper = &batch.docs[doc];
            sprout(
                &mut batch.nodes,
                doc,
                root,
                1,
                tree.root_node(),
                src,
                paper,
                spots,
            );
        }
        None => {
            for spot in spots {
                let label = tag(&batch.docs[doc], spot.0);
                leaf(&mut batch.nodes, doc, root, 1, label, vec![spot.0]);
            }
        }
    }
}

pub fn crown(doc: &Doc) -> String {
    let mut adds = 0;
    let mut cuts = 0;
    for cut in &doc.slices {
        adds += cut.text.len();
        cuts += cut.span.len();
    }
    if doc.fate == Fate::Gone {
        cuts = doc.base.len();
    }
    let mark = match doc.fate {
        Fate::Born => "new",
        Fate::Gone => "gone",
        Fate::Edit => "edit",
    };
    let bulk = match doc.atom {
        true => format!("{} bytes", doc.body.len()),
        false => format!("+{adds} -{cuts}"),
    };
    format!("{}  {mark} · {bulk}", doc.path)
}

pub fn tag(doc: &Doc, cut: usize) -> String {
    let slice = &doc.slices[cut];
    let peek = slice
        .text
        .iter()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .or_else(|| {
            doc.base
                .get(slice.span.clone())
                .and_then(|rows| rows.iter().find(|line| !line.trim().is_empty()))
                .map(|line| line.trim().to_string())
        })
        .unwrap_or_else(|| "whitespace".to_string());
    format!("+{} -{}  {}", slice.text.len(), slice.span.len(), peek)
}

pub fn leaf(
    nodes: &mut Vec<Node>,
    doc: usize,
    parent: usize,
    depth: usize,
    label: String,
    slices: Vec<usize>,
) -> usize {
    let index = nodes.len();
    nodes.push(Node {
        doc,
        parent: Some(parent),
        kids: Vec::new(),
        depth,
        last: false,
        label,
        slices,
        open: true,
        mark: None,
    });
    nodes[parent].kids.push(index);
    index
}

pub fn flow(nodes: &[Node], node: usize, rows: &mut Vec<usize>) {
    rows.push(node);
    if nodes[node].open {
        for kid in &nodes[node].kids {
            flow(nodes, *kid, rows);
        }
    }
}

pub fn wipe(nodes: &mut Vec<Node>, node: usize) {
    nodes[node].mark = None;
    for kid in nodes[node].kids.clone() {
        wipe(nodes, kid);
    }
}

pub fn gather(nodes: &[Node], node: usize, out: &mut Vec<usize>) {
    out.extend(nodes[node].slices.iter().copied());
    for kid in &nodes[node].kids {
        gather(nodes, *kid, out);
    }
}

pub fn weave(doc: &Doc, picks: &BTreeSet<usize>) -> String {
    let mut lines = doc.base.clone();
    let mut cuts: Vec<&Slice> = picks.iter().filter_map(|id| doc.slices.get(*id)).collect();

    cuts.sort_by(|a, b| {
        b.span
            .start
            .cmp(&a.span.start)
            .then(b.span.end.cmp(&a.span.end))
            .then(b.id.cmp(&a.id))
    });

    for cut in cuts {
        let lo = cut.span.start.min(lines.len());
        let hi = cut.span.end.min(lines.len()).max(lo);
        lines.splice(lo..hi, cut.text.iter().cloned());
    }

    let glue = match doc.eol {
        Eol::Lf => "\n",
        Eol::Crlf => "\r\n",
    };
    let mut out = lines.join(glue);
    if doc.tail && !out.is_empty() {
        out.push_str(glue);
    }
    out
}

pub fn refresh(repo: &gix::Repository, tree: ObjectId) -> Option<String> {
    let mut index = match repo.index_from_tree(&tree) {
        Ok(index) => index,
        Err(why) => {
            return Some(format!(
                "index build failed ({why}); run git reset"
            ));
        }
    };

    match index.write(gix::index::write::Options {
        extensions: Default::default(),
        skip_hash: false,
    }) {
        Ok(()) => None,
        Err(why) => Some(format!("index write failed ({why}); run git reset")),
    }
}
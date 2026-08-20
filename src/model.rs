use gix::ObjectId;
use gix::object::tree::EntryKind;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use tree_sitter::Language;

pub const CAP: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    Lf,
    Crlf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    Edit,
    Born,
    Gone,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    Tree,
    Diff,
    Kegs,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    Browse,
    Write,
    Find,
    Ask,
    Test,
    Log,
    Bail,
    Help,
}

pub struct Slice {
    pub id: usize,
    pub span: Range<usize>,
    pub fresh: Range<usize>,
    pub text: Vec<String>,
}

pub struct Doc {
    pub path: String,
    pub fate: Fate,
    pub mode: EntryKind,
    pub eol: Eol,
    pub tail: bool,
    pub atom: bool,
    pub base: Vec<String>,
    pub body: Vec<u8>,
    pub slices: Vec<Slice>,
    pub lang: Option<Language>,
    pub name: Option<String>,
}

pub struct Node {
    pub doc: usize,
    pub parent: Option<usize>,
    pub kids: Vec<usize>,
    pub depth: usize,
    pub last: bool,
    pub label: String,
    pub slices: Vec<usize>,
    pub open: bool,
    pub mark: Option<usize>,
}

#[derive(Clone, Default)]
pub struct Ledger {
    pub keeps: BTreeMap<usize, BTreeSet<usize>>,
    pub drops: BTreeSet<usize>,
}

impl Ledger {
    pub fn idle(&self) -> bool {
        self.keeps.is_empty() && self.drops.is_empty()
    }
}

pub struct Bucket {
    pub note: String,
    pub flaw: Option<String>,
    pub tally: usize,
    pub verify: Option<bool>,
}

pub struct Report {
    pub commits: Vec<(ObjectId, String)>,
    pub gripe: Option<String>,
}

pub struct Batch {
    pub docs: Vec<Doc>,
    pub nodes: Vec<Node>,
    pub roots: Vec<usize>,
    pub skips: Vec<String>,
}
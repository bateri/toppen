//! The process table as a person asks for it: sorted by a column, as a flat list, a tree or groups
//! of one name, narrowed by a filter, and held in place while a pointer is over it.
//!
//! The page never sees all of it: `Table::snapshot` hands over the rows in a window with some
//! padding around it, and says how many there are in all.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use super::filter::{Filter, Hits};
use crate::sample::Process;

/// How many rows are sent beyond each edge of the window, for a scroll to meet rows already there.
pub const PADDING: usize = 20;

/// How many characters of a command line a row carries.
const COMMAND_CHARS: usize = 400;

/// A process by its pid and the second it started in.
pub type Key = (u32, u64);

/// What a row is: a process, or a group of the processes of one name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Id {
    Process(Key),
    Group(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Pid,
    Name,
    User,
    Cpu,
    Memory,
    Virt,
    Threads,
    State,
    Nice,
    Time,
    Read,
    Write,
    Command,
}

impl Column {
    pub const ALL: [Column; 13] = [
        Column::Pid,
        Column::Name,
        Column::User,
        Column::Cpu,
        Column::Memory,
        Column::Virt,
        Column::Threads,
        Column::State,
        Column::Nice,
        Column::Time,
        Column::Read,
        Column::Write,
        Column::Command,
    ];

    /// The word the page and `docs/calls.md` use.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Column::Pid => "pid",
            Column::Name => "name",
            Column::User => "user",
            Column::Cpu => "cpu",
            Column::Memory => "memory",
            Column::Virt => "virt",
            Column::Threads => "threads",
            Column::State => "state",
            Column::Nice => "nice",
            Column::Time => "time",
            Column::Read => "read",
            Column::Write => "write",
            Column::Command => "command",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|column| column.word() == word)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Flat,
    Tree,
    Group,
}

impl Mode {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Mode::Flat => "flat",
            Mode::Tree => "tree",
            Mode::Group => "group",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        [Mode::Flat, Mode::Tree, Mode::Group]
            .into_iter()
            .find(|mode| mode.word() == word)
    }
}

/// Fold every branch, or open every branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    All,
    None,
}

/// What the page changes in the table; what is `None` stays as it was.
#[derive(Debug, Default)]
pub struct Change {
    pub sort: Option<Column>,
    pub descending: Option<bool>,
    pub mode: Option<Mode>,
    pub filter: Option<String>,
    pub hold: Option<bool>,
    /// `Some(None)` leaves the subtree.
    pub root: Option<Option<Key>>,
    /// The first row in view and how many rows are in view.
    pub window: Option<(usize, usize)>,
    /// `Some(None)` selects nothing.
    pub selected: Option<Option<Id>>,
    /// Fold or open this row once.
    pub toggle: Option<Id>,
    pub fold: Option<Fold>,
}

/// The table's state as the page last set it, to be told back.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub sort: Column,
    pub descending: bool,
    pub mode: Mode,
    pub filter: String,
    pub hold: bool,
    pub root: Option<Key>,
    /// Why the filter is not the one typed, where it is not.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: Id,
    /// A process as it is, or the totals of what a folded row hides; a group's totals.
    pub process: Process,
    pub depth: u16,
    /// What the row folds: the processes below it in a tree, the members of a group.
    pub children: usize,
    pub folded: bool,
    /// The process ended since the sample before; the row stays one sample, while the order is held.
    pub exited: bool,
    pub hits: Hits,
}

#[derive(Debug, Default)]
pub struct Snapshot {
    /// Processes the system has.
    pub total: usize,
    /// Processes that the filter lets through (below the root, where there is one).
    pub matched: usize,
    /// Rows in all, to size the scroll.
    pub visible: usize,
    /// How many rows are not where the order of the moment would put them.
    pub moved: usize,
    /// The order is the one held, not the one of the moment.
    pub held: bool,
    /// The index of `rows[0]`.
    pub first: usize,
    pub rows: Vec<Row>,
    /// Where the selected row is among all rows.
    pub selected_index: Option<usize>,
}

/// What a row folds adds up to.
#[derive(Debug, Clone, Copy, Default)]
struct Totals {
    cpu: f64,
    memory: u64,
    virt: u64,
    threads: u64,
    cpu_time: u64,
    read: f64,
    write: f64,
    count: usize,
}

impl Totals {
    fn of(process: &Process) -> Self {
        Self {
            cpu: process.cpu,
            memory: process.memory,
            virt: process.virt,
            threads: u64::from(process.threads),
            cpu_time: process.cpu_time,
            read: process.read,
            write: process.write,
            count: 1,
        }
    }

    fn add(&mut self, other: &Totals) {
        self.cpu += other.cpu;
        self.memory = self.memory.saturating_add(other.memory);
        self.virt = self.virt.saturating_add(other.virt);
        self.threads += other.threads;
        self.cpu_time = self.cpu_time.saturating_add(other.cpu_time);
        self.read += other.read;
        self.write += other.write;
        self.count += other.count;
    }

    fn put(&self, process: &mut Process) {
        process.cpu = self.cpu;
        process.memory = self.memory;
        process.virt = self.virt;
        process.threads = u32::try_from(self.threads).unwrap_or(u32::MAX);
        process.cpu_time = self.cpu_time;
        process.read = self.read;
        process.write = self.write;
    }
}

/// What one sample's processes come to, before they are ordered.
struct Context<'a> {
    procs: &'a [Process],
    /// `Some` for a process below the root that the filter lets through.
    hits: Vec<Option<Hits>>,
    /// Below the root (all of them where there is none).
    eligible: Vec<bool>,
    /// The index of the parent process, where it is one.
    parent: Vec<Option<usize>>,
    kids: Vec<Vec<usize>>,
}

#[derive(Debug)]
pub struct Table {
    sort: Column,
    descending: bool,
    mode: Mode,
    filter_text: String,
    filter: Filter,
    /// The ports each process listens on (by pid), as the last scan said; only kept up while the
    /// filter asks about ports.
    ports: HashMap<u32, Vec<u16>>,
    error: Option<String>,
    hold: bool,
    root: Option<Key>,
    window: (usize, usize),
    selected: Option<Id>,
    /// Branches of the tree that are folded.
    collapsed: HashSet<Key>,
    /// Groups that are open.
    expanded: HashSet<String>,
    fold: Option<Fold>,
    /// The sample the last snapshot was of.
    at: Option<f64>,
    /// The rows as they were last sent, but for those that ended.
    shown: Vec<Id>,
    /// Rows of processes that ended, by where in `shown` they were.
    exited: Vec<(usize, Process)>,
    /// The processes of the sample before, kept while the order is held in a flat list.
    last: HashMap<Key, Process>,
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

/// The order of two text values, whatever the case.
fn text(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

/// The length of the longest run of `values` that only grows.
fn longest_run(values: &[usize]) -> usize {
    let mut tails: Vec<usize> = Vec::new();
    for &value in values {
        let at = tails.partition_point(|&tail| tail < value);
        if at == tails.len() {
            tails.push(value);
        } else {
            tails[at] = value;
        }
    }
    tails.len()
}

/// How many of `held` are out of place compared with `natural`: rows that are not in the longest
/// run that both orders agree on (a row that jumps far away counts as one).
fn moved_between(natural: &[Row], held: &[Row]) -> usize {
    let rank: HashMap<&Id, usize> = natural
        .iter()
        .enumerate()
        .map(|(at, row)| (&row.id, at))
        .collect();
    let ranks: Vec<usize> = held
        .iter()
        .filter_map(|row| rank.get(&row.id).copied())
        .collect();
    ranks.len() - longest_run(&ranks)
}

impl Table {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sort: Column::Cpu,
            descending: true,
            mode: Mode::Flat,
            filter_text: String::new(),
            filter: Filter::default(),
            ports: HashMap::new(),
            error: None,
            hold: false,
            root: None,
            window: (0, 50),
            selected: None,
            collapsed: HashSet::new(),
            expanded: HashSet::new(),
            fold: None,
            at: None,
            shown: Vec::new(),
            exited: Vec::new(),
            last: HashMap::new(),
        }
    }

    /// Whether the filter asks about ports, so that a scan is worth making before the next snapshot.
    #[must_use]
    pub fn wants_ports(&self) -> bool {
        self.filter.uses_ports()
    }

    /// What the last scan said of the ports each process listens on.
    pub fn set_ports(&mut self, ports: HashMap<u32, Vec<u16>>) {
        self.ports = ports;
    }

    #[must_use]
    pub fn view(&self) -> View {
        View {
            sort: self.sort,
            descending: self.descending,
            mode: self.mode,
            filter: self.filter_text.clone(),
            hold: self.hold,
            root: self.root,
            error: self.error.clone(),
        }
    }

    /// What the order is forgotten by: another column, another shape, another set of rows.
    fn reorder(&mut self) {
        self.shown.clear();
        self.exited.clear();
        self.last.clear();
    }

    pub fn apply(&mut self, change: Change) {
        if let Some(sort) = change.sort {
            if sort != self.sort {
                self.sort = sort;
                self.reorder();
            }
        }
        if let Some(descending) = change.descending {
            if descending != self.descending {
                self.descending = descending;
                self.reorder();
            }
        }
        if let Some(mode) = change.mode {
            if mode != self.mode {
                self.mode = mode;
                self.reorder();
            }
        }
        if let Some(text) = change.filter {
            if text != self.filter_text {
                match Filter::parse(&text) {
                    Ok(filter) => {
                        self.filter = filter;
                        self.error = None;
                    }
                    // The last filter that was good stays in force, so that a half-typed pattern does not
                    // empty the table.
                    Err(why) => self.error = Some(why),
                }
                self.filter_text = text;
                self.reorder();
            }
        }
        if let Some(hold) = change.hold {
            self.hold = hold;
            if !hold {
                self.exited.clear();
                self.last.clear();
            }
        }
        if let Some(root) = change.root {
            if root != self.root {
                self.root = root;
                self.reorder();
            }
        }
        if let Some(window) = change.window {
            self.window = window;
        }
        if let Some(selected) = change.selected {
            self.selected = selected;
        }
        if let Some(id) = change.toggle {
            match id {
                Id::Process(key) => {
                    if !self.collapsed.remove(&key) {
                        self.collapsed.insert(key);
                    }
                }
                Id::Group(name) => {
                    if !self.expanded.remove(&name) {
                        self.expanded.insert(name);
                    }
                }
            }
        }
        if let Some(fold) = change.fold {
            self.fold = Some(fold);
        }
    }

    fn context<'a>(&mut self, procs: &'a [Process]) -> Context<'a> {
        let count = procs.len();
        let by_pid: HashMap<u32, usize> = procs
            .iter()
            .enumerate()
            .map(|(at, process)| (process.pid, at))
            .collect();
        // A parent started no later than its child: a pid that was used again is not the parent.
        let parent: Vec<Option<usize>> = procs
            .iter()
            .enumerate()
            .map(|(at, process)| {
                by_pid
                    .get(&process.ppid)
                    .copied()
                    .filter(|&up| up != at && procs[up].start <= process.start)
            })
            .collect();
        let mut kids = vec![Vec::new(); count];
        for (at, up) in parent.iter().enumerate() {
            if let Some(up) = up {
                kids[*up].push(at);
            }
        }
        let mut eligible = vec![true; count];
        if let Some(root) = self.root {
            match procs.iter().position(|process| process.key() == root) {
                Some(top) => {
                    eligible = vec![false; count];
                    let mut stack = vec![top];
                    eligible[top] = true;
                    while let Some(at) = stack.pop() {
                        for &kid in &kids[at] {
                            if !eligible[kid] {
                                eligible[kid] = true;
                                stack.push(kid);
                            }
                        }
                    }
                }
                // The process the table was narrowed to has ended.
                None => self.root = None,
            }
        }
        let hits = procs
            .iter()
            .enumerate()
            .map(|(at, process)| {
                if eligible[at] {
                    let ports = self.ports.get(&process.pid).map_or(&[][..], Vec::as_slice);
                    self.filter.matches(process, ports)
                } else {
                    None
                }
            })
            .collect();
        Context {
            procs,
            hits,
            eligible,
            parent,
            kids,
        }
    }

    fn compare(&self, held: Option<&HashMap<Id, usize>>, a: &Row, b: &Row) -> Ordering {
        if let Some(held) = held {
            match (held.get(&a.id), held.get(&b.id)) {
                (Some(x), Some(y)) => return x.cmp(y),
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => {}
            }
        }
        let (a, b) = (&a.process, &b.process);
        let ordering = match self.sort {
            Column::Pid => a.pid.cmp(&b.pid),
            Column::Name => text(&a.name, &b.name),
            Column::User => text(&a.user, &b.user),
            Column::Cpu => a.cpu.total_cmp(&b.cpu),
            Column::Memory => a.memory.cmp(&b.memory),
            Column::Virt => a.virt.cmp(&b.virt),
            Column::Threads => a.threads.cmp(&b.threads),
            Column::State => a.state.cmp(&b.state),
            Column::Nice => a.nice.cmp(&b.nice),
            Column::Time => a.cpu_time.cmp(&b.cpu_time),
            Column::Read => a.read.total_cmp(&b.read),
            Column::Write => a.write.total_cmp(&b.write),
            Column::Command => text(&a.command, &b.command),
        };
        let ordering = if self.descending {
            ordering.reverse()
        } else {
            ordering
        };
        // Equals keep one order in both directions.
        ordering
            .then(a.pid.cmp(&b.pid))
            .then(a.start.cmp(&b.start))
            .then_with(|| a.name.cmp(&b.name))
    }

    fn row(process: &Process, hits: Hits) -> Row {
        Row {
            id: Id::Process(process.key()),
            process: process.clone(),
            depth: 0,
            children: 0,
            folded: false,
            exited: false,
            hits,
        }
    }

    fn flat(&self, context: &Context, held: Option<&HashMap<Id, usize>>) -> Vec<Row> {
        let mut rows: Vec<Row> = context
            .procs
            .iter()
            .zip(&context.hits)
            .filter_map(|(process, hits)| hits.clone().map(|hits| Self::row(process, hits)))
            .collect();
        rows.sort_by(|a, b| self.compare(held, a, b));
        rows
    }

    fn tree(&self, context: &Context, held: Option<&HashMap<Id, usize>>) -> Vec<Row> {
        let Context {
            procs,
            hits,
            eligible,
            parent,
            kids,
        } = context;
        let count = procs.len();
        // Parents before children: the roots are what has no parent below the root, and what a loop
        // of parents leaves out comes after.
        let mut order = Vec::with_capacity(count);
        let mut up: Vec<Option<usize>> = vec![None; count];
        let mut seen = vec![false; count];
        let roots: Vec<usize> = (0..count)
            .filter(|&at| eligible[at] && parent[at].is_none_or(|top| !eligible[top]))
            .collect();
        for start in roots.into_iter().chain(0..count) {
            if !eligible[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            let mut stack = vec![start];
            while let Some(at) = stack.pop() {
                order.push(at);
                for &kid in &kids[at] {
                    if eligible[kid] && !seen[kid] {
                        seen[kid] = true;
                        up[kid] = Some(at);
                        stack.push(kid);
                    }
                }
            }
        }
        // What a branch adds up to, and which branches hold a match.
        let mut totals: Vec<Totals> = procs.iter().map(Totals::of).collect();
        let mut kept: Vec<bool> = hits.iter().map(Option::is_some).collect();
        for &at in order.iter().rev() {
            if let Some(top) = up[at] {
                let below = totals[at];
                totals[top].add(&below);
                if kept[at] {
                    kept[top] = true;
                }
            }
        }
        let mut rows: Vec<Row> = Vec::new();
        let mut slot = vec![usize::MAX; count];
        for &at in &order {
            if !kept[at] {
                continue;
            }
            let process = &procs[at];
            let mut row = Self::row(process, hits[at].clone().unwrap_or_default());
            row.children = totals[at].count - 1;
            row.folded = row.children > 0 && self.collapsed.contains(&process.key());
            if row.folded {
                totals[at].put(&mut row.process);
            }
            slot[at] = rows.len();
            rows.push(row);
        }
        let mut down: Vec<Vec<usize>> = vec![Vec::new(); rows.len()];
        let mut tops: Vec<usize> = Vec::new();
        for &at in &order {
            if !kept[at] {
                continue;
            }
            match up[at] {
                Some(top) => down[slot[top]].push(slot[at]),
                None => tops.push(slot[at]),
            }
        }
        let by = |x: &usize, y: &usize| self.compare(held, &rows[*x], &rows[*y]);
        tops.sort_by(by);
        for list in &mut down {
            list.sort_by(by);
        }
        let mut cells: Vec<Option<Row>> = rows.into_iter().map(Some).collect();
        let mut out = Vec::with_capacity(cells.len());
        let mut stack: Vec<(usize, u16)> = tops.into_iter().rev().map(|at| (at, 0)).collect();
        while let Some((at, depth)) = stack.pop() {
            let Some(mut row) = cells[at].take() else {
                continue;
            };
            row.depth = depth;
            if !row.folded {
                stack.extend(
                    down[at]
                        .iter()
                        .rev()
                        .map(|&kid| (kid, depth.saturating_add(1))),
                );
            }
            out.push(row);
        }
        out
    }

    fn groups(&self, context: &Context, held: Option<&HashMap<Id, usize>>) -> Vec<Row> {
        let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
        for (at, hit) in context.hits.iter().enumerate() {
            if hit.is_some() {
                by_name
                    .entry(context.procs[at].name.as_str())
                    .or_default()
                    .push(at);
            }
        }
        let mut heads: Vec<(Row, Vec<Row>)> = by_name
            .into_iter()
            .map(|(name, members)| {
                let mut totals = Totals::default();
                for &at in &members {
                    totals.add(&Totals::of(&context.procs[at]));
                }
                let user = {
                    let first = &context.procs[members[0]].user;
                    if members.iter().all(|&at| context.procs[at].user == *first) {
                        first.clone()
                    } else {
                        String::new()
                    }
                };
                let mut process = context.procs[members[0]].clone();
                process.pid = 0;
                process.ppid = 0;
                process.start = 0;
                process.name = name.to_owned();
                process.user = user;
                process.command = String::new();
                process.state = ' ';
                process.nice = 0;
                totals.put(&mut process);
                let open = self.expanded.contains(name);
                let head = Row {
                    id: Id::Group(name.to_owned()),
                    process,
                    depth: 0,
                    children: members.len(),
                    folded: !open,
                    exited: false,
                    hits: Hits::default(),
                };
                let mut below: Vec<Row> = if open {
                    members
                        .iter()
                        .map(|&at| {
                            let mut row = Self::row(
                                &context.procs[at],
                                context.hits[at].clone().unwrap_or_default(),
                            );
                            row.depth = 1;
                            row
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                below.sort_by(|a, b| self.compare(held, a, b));
                (head, below)
            })
            .collect();
        heads.sort_by(|(a, _), (b, _)| self.compare(held, a, b));
        heads
            .into_iter()
            .flat_map(|(head, below)| std::iter::once(head).chain(below))
            .collect()
    }

    fn layout(&self, context: &Context, held: Option<&HashMap<Id, usize>>) -> Vec<Row> {
        match self.mode {
            Mode::Flat => self.flat(context, held),
            Mode::Tree => self.tree(context, held),
            Mode::Group => self.groups(context, held),
        }
    }

    /// Folds and opens every branch, as asked; it takes the processes to know which are branches.
    fn fold_all(&mut self, procs: &[Process]) {
        let Some(fold) = self.fold.take() else { return };
        match (self.mode, fold) {
            (Mode::Group, Fold::All) => self.expanded.clear(),
            (Mode::Group, Fold::None) => {
                self.expanded = procs.iter().map(|process| process.name.clone()).collect()
            }
            (_, Fold::None) => self.collapsed.clear(),
            (_, Fold::All) => {
                let pids: HashMap<u32, Key> = procs
                    .iter()
                    .map(|process| (process.pid, process.key()))
                    .collect();
                self.collapsed = procs
                    .iter()
                    .filter_map(|process| pids.get(&process.ppid).copied())
                    .filter(|&(pid, start)| {
                        procs
                            .iter()
                            .any(|process| process.pid == pid && process.start == start)
                    })
                    .collect();
            }
        }
    }

    /// The table of `procs`, the sample of the second `at`.
    pub fn snapshot(&mut self, procs: &[Process], at: f64) -> Snapshot {
        let fresh = self.at != Some(at);
        self.at = Some(at);
        if fresh {
            // What ended is no longer there to fold.
            let keys: HashSet<Key> = procs.iter().map(Process::key).collect();
            self.collapsed.retain(|key| keys.contains(key));
            let names: HashSet<&str> = procs.iter().map(|process| process.name.as_str()).collect();
            self.expanded.retain(|name| names.contains(name.as_str()));
        }
        self.fold_all(procs);
        let context = self.context(procs);
        let matched = context.hits.iter().filter(|hit| hit.is_some()).count();

        // Rows of processes that ended since the sample before, while the order is held.
        let keeps = self.hold && self.mode == Mode::Flat;
        if fresh {
            self.exited.clear();
            if keeps && !self.last.is_empty() {
                let present: HashSet<Key> = procs.iter().map(Process::key).collect();
                for (index, id) in self.shown.iter().enumerate() {
                    if let Id::Process(key) = id {
                        if !present.contains(key) {
                            if let Some(process) = self.last.get(key) {
                                self.exited.push((index, process.clone()));
                            }
                        }
                    }
                }
            }
            self.last = if keeps {
                procs
                    .iter()
                    .map(|process| (process.key(), process.clone()))
                    .collect()
            } else {
                HashMap::new()
            };
        }

        let natural = self.layout(&context, None);
        let held = self.hold && !self.shown.is_empty();
        let (mut list, moved) = if held {
            let positions: HashMap<Id, usize> = self
                .shown
                .iter()
                .cloned()
                .enumerate()
                .map(|(at, id)| (id, at))
                .collect();
            let list = self.layout(&context, Some(&positions));
            let moved = moved_between(&natural, &list);
            (list, moved)
        } else {
            (natural, 0)
        };
        self.shown = list.iter().map(|row| row.id.clone()).collect();
        if keeps {
            for (index, process) in &self.exited {
                let mut row = Self::row(process, Hits::default());
                row.exited = true;
                list.insert((*index).min(list.len()), row);
            }
        }

        let visible = list.len();
        let selected_index = self
            .selected
            .as_ref()
            .and_then(|id| list.iter().position(|row| row.id == *id));
        let first = self.window.0.min(visible).saturating_sub(PADDING);
        let end = self
            .window
            .0
            .saturating_add(self.window.1)
            .saturating_add(PADDING)
            .min(visible);
        let mut rows: Vec<Row> = list.drain(first..end.max(first)).collect();
        for row in &mut rows {
            if row.process.command.chars().count() > COMMAND_CHARS {
                row.process.command = row.process.command.chars().take(COMMAND_CHARS).collect();
            }
        }
        Snapshot {
            total: procs.len(),
            matched,
            visible,
            moved,
            held,
            first,
            rows,
            selected_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, ppid: u32, name: &str, cpu: f64, memory: u64) -> Process {
        Process {
            pid,
            ppid,
            start: 1000 + u64::from(pid),
            name: name.to_owned(),
            user: "lantern".to_owned(),
            command: format!("/bin/{name} --pid {pid}"),
            cpu,
            memory,
            virt: memory * 2,
            threads: 2,
            state: 'S',
            nice: 0,
            cpu_time: u64::from(pid) * 10,
            read: 0.0,
            write: 0.0,
        }
    }

    fn pids(snapshot: &Snapshot) -> Vec<u32> {
        snapshot.rows.iter().map(|row| row.process.pid).collect()
    }

    fn sample() -> Vec<Process> {
        vec![
            process(1, 0, "init", 0.5, 10 << 20),
            process(10, 1, "shell", 1.0, 20 << 20),
            process(11, 10, "editor", 30.0, 300 << 20),
            process(12, 10, "compiler", 80.0, 900 << 20),
            process(13, 12, "linker", 10.0, 100 << 20),
            process(20, 1, "daemon", 2.0, 40 << 20),
        ]
    }

    fn change() -> Change {
        Change::default()
    }

    fn with(change: Change) -> Table {
        let mut table = Table::new();
        table.apply(change);
        table
    }

    #[test]
    fn a_flat_table_is_ordered_by_the_column_and_equals_by_pid() {
        let mut table = Table::new();
        let snapshot = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&snapshot), [12, 11, 13, 20, 10, 1]);
        assert_eq!(
            (snapshot.total, snapshot.matched, snapshot.visible),
            (6, 6, 6)
        );

        let mut table = with(Change {
            sort: Some(Column::Name),
            descending: Some(false),
            ..change()
        });
        assert_eq!(
            pids(&table.snapshot(&sample(), 1.0)),
            [12, 20, 11, 1, 13, 10]
        );
        table.apply(Change {
            descending: Some(true),
            ..change()
        });
        assert_eq!(
            pids(&table.snapshot(&sample(), 1.0)),
            [10, 13, 1, 11, 20, 12]
        );

        // Threads are all equal: the pid decides, in both directions.
        let mut table = with(Change {
            sort: Some(Column::Threads),
            descending: Some(false),
            ..change()
        });
        assert_eq!(
            pids(&table.snapshot(&sample(), 1.0)),
            [1, 10, 11, 12, 13, 20]
        );
        table.apply(Change {
            descending: Some(true),
            ..change()
        });
        assert_eq!(
            pids(&table.snapshot(&sample(), 1.0)),
            [1, 10, 11, 12, 13, 20]
        );
    }

    #[test]
    fn every_column_orders() {
        for column in Column::ALL {
            assert_eq!(Column::parse(column.word()), Some(column));
            let mut table = with(Change {
                sort: Some(column),
                ..change()
            });
            let snapshot = table.snapshot(&sample(), 1.0);
            assert_eq!(snapshot.rows.len(), 6, "{}", column.word());
        }
        assert_eq!(Column::parse("nope"), None);
        assert_eq!(Mode::parse("tree"), Some(Mode::Tree));
    }

    #[test]
    fn a_tree_puts_children_below_their_parents_and_sorts_siblings() {
        let mut table = with(Change {
            mode: Some(Mode::Tree),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 1.0);
        // The daemon is busier than the shell, so it comes first; the compiler before the editor.
        assert_eq!(pids(&snapshot), [1, 20, 10, 12, 13, 11]);
        let depths: Vec<u16> = snapshot.rows.iter().map(|row| row.depth).collect();
        assert_eq!(depths, [0, 1, 1, 2, 3, 2]);
        let branches: Vec<usize> = snapshot.rows.iter().map(|row| row.children).collect();
        assert_eq!(branches, [5, 0, 3, 1, 0, 0]);
    }

    #[test]
    fn a_folded_branch_carries_what_it_hides() {
        let mut table = with(Change {
            mode: Some(Mode::Tree),
            toggle: Some(Id::Process((10, 1010))),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&snapshot), [1, 10, 20]);
        let folded = &snapshot.rows[1];
        assert!(folded.folded);
        assert_eq!(folded.children, 3);
        assert!(
            (folded.process.cpu - 121.0).abs() < 1e-9,
            "{}",
            folded.process.cpu
        );
        assert_eq!(folded.process.memory, (20 + 300 + 900 + 100) << 20);
        assert_eq!(folded.process.threads, 8);
        // The totals decide where it sorts among its siblings.
        let mut table = with(Change {
            mode: Some(Mode::Tree),
            toggle: Some(Id::Process((10, 1010))),
            ..change()
        });
        let rows = table.snapshot(&sample(), 1.0).rows;
        assert!(rows[1].process.cpu > rows[2].process.cpu);
        // Opening it again shows the rows again.
        table.apply(Change {
            toggle: Some(Id::Process((10, 1010))),
            ..change()
        });
        assert_eq!(table.snapshot(&sample(), 2.0).visible, 6);
    }

    #[test]
    fn fold_all_and_open_all() {
        let mut table = with(Change {
            mode: Some(Mode::Tree),
            fold: Some(Fold::All),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&snapshot), [1]);
        assert_eq!(snapshot.rows[0].children, 5);
        table.apply(Change {
            fold: Some(Fold::None),
            ..change()
        });
        assert_eq!(table.snapshot(&sample(), 1.0).visible, 6);
    }

    #[test]
    fn a_pid_used_again_is_not_a_parent_and_a_loop_of_parents_still_shows() {
        let mut procs = sample();
        // Process 11's parent pid 10 now names a process that started after it.
        procs[1].start = 5000;
        let mut table = with(Change {
            mode: Some(Mode::Tree),
            ..change()
        });
        let snapshot = table.snapshot(&procs, 1.0);
        let row = snapshot
            .rows
            .iter()
            .find(|row| row.process.pid == 11)
            .unwrap();
        assert_eq!(row.depth, 0);
        // Two processes that name each other.
        let mut procs = sample();
        procs[4].ppid = 12;
        procs[3].ppid = 13;
        procs[3].start = procs[4].start;
        let snapshot = table.snapshot(&procs, 2.0);
        assert_eq!(snapshot.visible, 6);
    }

    #[test]
    fn groups_of_one_name_carry_totals_and_open_into_their_members() {
        let mut procs = sample();
        procs.push(process(30, 1, "worker", 5.0, 50 << 20));
        procs.push(process(31, 1, "worker", 7.0, 70 << 20));
        procs.push(process(32, 1, "worker", 3.0, 30 << 20));
        let mut table = with(Change {
            mode: Some(Mode::Group),
            ..change()
        });
        let snapshot = table.snapshot(&procs, 1.0);
        assert_eq!(snapshot.visible, 6 + 1);
        let worker = snapshot
            .rows
            .iter()
            .find(|row| row.id == Id::Group("worker".to_owned()))
            .unwrap();
        assert_eq!(
            (worker.children, worker.folded, worker.process.pid),
            (3, true, 0)
        );
        assert!((worker.process.cpu - 15.0).abs() < 1e-9);
        assert_eq!(worker.process.user, "lantern");
        table.apply(Change {
            toggle: Some(Id::Group("worker".to_owned())),
            ..change()
        });
        let snapshot = table.snapshot(&procs, 2.0);
        assert_eq!(snapshot.visible, 6 + 1 + 3);
        let at = snapshot
            .rows
            .iter()
            .position(|row| row.id == Id::Group("worker".to_owned()))
            .unwrap();
        let below: Vec<(u32, u16)> = snapshot.rows[at + 1..at + 4]
            .iter()
            .map(|row| (row.process.pid, row.depth))
            .collect();
        assert_eq!(below, [(31, 1), (30, 1), (32, 1)]);
        // Different users leave the group without one.
        procs[7].user = "root".to_owned();
        table.apply(Change {
            toggle: Some(Id::Group("worker".to_owned())),
            ..change()
        });
        let snapshot = table.snapshot(&procs, 3.0);
        let worker = snapshot
            .rows
            .iter()
            .find(|row| row.id == Id::Group("worker".to_owned()))
            .unwrap();
        assert_eq!(worker.process.user, "");
    }

    #[test]
    fn the_filter_narrows_every_shape_and_a_tree_keeps_the_way_to_a_match() {
        let mut table = with(Change {
            filter: Some("linker".to_owned()),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&snapshot), [13]);
        assert_eq!((snapshot.total, snapshot.matched), (6, 1));
        assert!(!snapshot.rows[0].hits.name.is_empty());

        table.apply(Change {
            mode: Some(Mode::Tree),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&snapshot), [1, 10, 12, 13]);
        assert_eq!(snapshot.matched, 1);
        assert!(snapshot.rows[0].hits.name.is_empty());

        table.apply(Change {
            mode: Some(Mode::Group),
            ..change()
        });
        assert_eq!(pids(&table.snapshot(&sample(), 1.0)), [0]);
    }

    #[test]
    fn a_filter_that_is_wrong_leaves_the_last_good_one_and_says_why() {
        let mut table = with(Change {
            filter: Some("linker".to_owned()),
            ..change()
        });
        table.apply(Change {
            filter: Some("/(linker/".to_owned()),
            ..change()
        });
        let view = table.view();
        assert_eq!(view.filter, "/(linker/");
        assert!(
            view.error
                .as_deref()
                .is_some_and(|why| why.starts_with("not a regular expression")),
            "{:?}",
            view.error
        );
        assert_eq!(pids(&table.snapshot(&sample(), 1.0)), [13]);
        table.apply(Change {
            filter: Some(String::new()),
            ..change()
        });
        assert_eq!(table.view().error, None);
        assert_eq!(table.snapshot(&sample(), 1.0).visible, 6);
    }

    #[test]
    fn narrowing_to_a_subtree_keeps_the_process_and_what_is_below_it() {
        let mut table = with(Change {
            root: Some(Some((12, 1012))),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&snapshot), [12, 13]);
        assert_eq!((snapshot.total, snapshot.matched), (6, 2));
        table.apply(Change {
            mode: Some(Mode::Tree),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&snapshot), [12, 13]);
        assert_eq!(snapshot.rows[0].depth, 0);
        // The process ended: the table is whole again.
        let mut procs = sample();
        procs.retain(|process| process.pid != 12);
        assert_eq!(table.snapshot(&procs, 2.0).visible, 5);
        assert_eq!(table.view().root, None);
    }

    #[test]
    fn held_rows_keep_their_places_new_rows_go_last_and_moved_counts_what_the_order_would_move() {
        let mut table = Table::new();
        table.apply(Change {
            hold: Some(true),
            ..change()
        });
        let first = table.snapshot(&sample(), 1.0);
        assert_eq!(pids(&first), [12, 11, 13, 20, 10, 1]);
        assert_eq!((first.held, first.moved), (false, 0));

        // The editor takes over the lead, and a new process appears.
        let mut procs = sample();
        procs[2].cpu = 95.0;
        procs.push(process(40, 1, "fresh", 99.0, 1 << 20));
        let second = table.snapshot(&procs, 2.0);
        assert!(second.held);
        assert_eq!(pids(&second), [12, 11, 13, 20, 10, 1, 40]);
        // The natural order is 40, 11, 12, 13, ...: the held one is not on the longest run.
        assert!(second.moved >= 2, "{}", second.moved);

        // Letting go gives the natural order back.
        table.apply(Change {
            hold: Some(false),
            ..change()
        });
        let third = table.snapshot(&procs, 3.0);
        assert_eq!((third.held, third.moved), (false, 0));
        assert_eq!(pids(&third), [40, 11, 12, 13, 20, 10, 1]);
    }

    #[test]
    fn an_exited_row_stays_one_sample_while_the_order_is_held() {
        let mut table = Table::new();
        table.apply(Change {
            hold: Some(true),
            ..change()
        });
        table.snapshot(&sample(), 1.0);
        let mut procs = sample();
        procs.retain(|process| process.pid != 11);
        let second = table.snapshot(&procs, 2.0);
        assert_eq!(pids(&second), [12, 11, 13, 20, 10, 1]);
        let exited: Vec<bool> = second.rows.iter().map(|row| row.exited).collect();
        assert_eq!(exited, [false, true, false, false, false, false]);
        // Asked again for the same sample, it is still there; the sample after, it is gone.
        assert_eq!(table.snapshot(&procs, 2.0).visible, 6);
        let third = table.snapshot(&procs, 3.0);
        assert_eq!(pids(&third), [12, 13, 20, 10, 1]);
        assert!(third.rows.iter().all(|row| !row.exited));
    }

    #[test]
    fn a_change_of_column_forgets_the_held_order() {
        let mut table = Table::new();
        table.apply(Change {
            hold: Some(true),
            ..change()
        });
        table.snapshot(&sample(), 1.0);
        table.apply(Change {
            sort: Some(Column::Pid),
            descending: Some(false),
            ..change()
        });
        let snapshot = table.snapshot(&sample(), 2.0);
        assert_eq!(pids(&snapshot), [1, 10, 11, 12, 13, 20]);
        assert_eq!(snapshot.moved, 0);
    }

    #[test]
    fn the_window_comes_with_padding_and_is_cut_at_the_ends() {
        let procs: Vec<Process> = (1..=2000)
            .map(|pid| process(pid, 0, "many", f64::from(pid), 1 << 20))
            .collect();
        let mut table = with(Change {
            window: Some((100, 30)),
            selected: Some(Some(Id::Process((1990, 2990)))),
            ..change()
        });
        let snapshot = table.snapshot(&procs, 1.0);
        assert_eq!(snapshot.visible, 2000);
        assert_eq!((snapshot.first, snapshot.rows.len()), (80, 70));
        // Highest cpu first: row index 10 is pid 1990.
        assert_eq!(snapshot.selected_index, Some(10));
        assert_eq!(snapshot.rows[0].process.pid, 2000 - 80);
        table.apply(Change {
            window: Some((0, 30)),
            ..change()
        });
        let snapshot = table.snapshot(&procs, 1.0);
        assert_eq!((snapshot.first, snapshot.rows.len()), (0, 50));
        table.apply(Change {
            window: Some((1990, 30)),
            ..change()
        });
        let snapshot = table.snapshot(&procs, 1.0);
        assert_eq!((snapshot.first, snapshot.rows.len()), (1970, 30));
        table.apply(Change {
            window: Some((5000, 30)),
            ..change()
        });
        // Beyond the end the window is the last rows.
        let snapshot = table.snapshot(&procs, 1.0);
        assert_eq!((snapshot.first, snapshot.rows.len()), (1980, 20));
        // A selection that is gone is not found.
        table.apply(Change {
            selected: Some(Some(Id::Process((7, 7)))),
            ..change()
        });
        assert_eq!(table.snapshot(&procs, 1.0).selected_index, None);
    }

    #[test]
    fn a_long_command_is_cut() {
        let mut procs = sample();
        procs[0].command = "x".repeat(1000);
        let mut table = Table::new();
        let snapshot = table.snapshot(&procs, 1.0);
        let row = snapshot
            .rows
            .iter()
            .find(|row| row.process.pid == 1)
            .unwrap();
        assert_eq!(row.process.command.chars().count(), COMMAND_CHARS);
    }

    #[test]
    fn the_longest_run_counts_what_stays_in_order() {
        assert_eq!(longest_run(&[]), 0);
        assert_eq!(longest_run(&[0, 1, 2, 3]), 4);
        assert_eq!(longest_run(&[3, 0, 1, 2]), 3);
        assert_eq!(longest_run(&[3, 2, 1, 0]), 1);
        assert_eq!(longest_run(&[1, 2, 0, 3, 4]), 4);
    }
}

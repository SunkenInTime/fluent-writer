//! A word-level diff for showing a proposed version against the draft, and tracking of a frozen
//! range through later edits (Swift `WordDiff` and `FrozenScope`).

use std::collections::HashSet;
use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffSegmentKind {
    Equal,
    Insert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSegment {
    pub kind: DiffSegmentKind,
    pub text: String,
}

impl DiffSegment {
    pub fn new(kind: DiffSegmentKind, text: impl Into<String>) -> Self {
        DiffSegment {
            kind,
            text: text.into(),
        }
    }
}

// ICU `\s` is [\t\n\f\r\p{Z}].
static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\t\n\x0C\r\p{Z}]+|[\p{L}\p{N}'’]+|[^\t\n\x0C\r\p{Z}\p{L}\p{N}]")
        .expect("valid pattern")
});

fn tokens(s: &str) -> Vec<&str> {
    TOKEN.find_iter(s).map(|m| m.as_str()).collect()
}

enum Change {
    Remove(usize),
    Insert(usize),
}

/// Swift's `CollectionDifference` (`b.difference(from: a)`): the Myers diff as the standard library
/// implements it, including its tie-breaking, so the same edits come out.
fn myers(a: &[&str], b: &[&str]) -> Vec<Change> {
    let n = a.len() as isize;
    let m = b.len() as isize;
    let max = n + m;
    // V arrays indexed by k in -d...d, stored with an offset.
    struct V {
        offset: isize,
        data: Vec<isize>,
    }
    impl V {
        fn new(max_index: isize) -> V {
            V {
                offset: max_index,
                data: vec![0; (2 * max_index + 1) as usize],
            }
        }
        fn get(&self, k: isize) -> isize {
            self.data[(k + self.offset) as usize]
        }
        fn set(&mut self, k: isize, v: isize) {
            self.data[(k + self.offset) as usize] = v;
        }
    }

    let mut trace: Vec<V> = Vec::new();
    let mut v = V::new(1);
    v.set(1, 0);
    let mut x: isize = 0;
    let mut y: isize = 0;
    'iterator: for d in 0..=max {
        let prev_v = v;
        v = V::new(d);
        let mut k = -d;
        while k <= d {
            if k == -d {
                x = prev_v.get(k + 1);
            } else {
                let km = prev_v.get(k - 1);
                if k != d {
                    let kp = prev_v.get(k + 1);
                    x = if km < kp { kp } else { km + 1 };
                } else {
                    x = km + 1;
                }
            }
            y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v.set(k, x);
            if x >= n && y >= m {
                trace.push(prev_v);
                break 'iterator;
            }
            k += 2;
        }
        trace.push(prev_v);
    }
    debug_assert!(x >= n && y >= m);

    let mut changes = Vec::new();
    let mut x = n;
    let mut y = m;
    let mut d = trace.len() as isize - 1;
    while d > 0 {
        let v = &trace[d as usize];
        let k = x - y;
        let prev_k = if k == -d || (k != d && v.get(k - 1) < v.get(k + 1)) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v.get(prev_k);
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
        }
        if y != prev_y {
            changes.push(Change::Insert(prev_y as usize));
        } else {
            changes.push(Change::Remove(prev_x as usize));
        }
        x = prev_x;
        y = prev_y;
        d -= 1;
    }
    changes
}

pub fn diff(old: &str, new: &str) -> Vec<DiffSegment> {
    let a = tokens(old);
    let b = tokens(new);
    let mut removed = HashSet::new();
    let mut inserted = HashSet::new();
    for change in myers(&a, &b) {
        match change {
            Change::Remove(o) => removed.insert(o),
            Change::Insert(o) => inserted.insert(o),
        };
    }
    let mut segments: Vec<DiffSegment> = Vec::new();
    let mut push = |kind: DiffSegmentKind, text: &str| {
        if let Some(last) = segments.last_mut()
            && last.kind == kind
        {
            last.text.push_str(text);
            return;
        }
        segments.push(DiffSegment::new(kind, text));
    };
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && removed.contains(&i) {
            push(DiffSegmentKind::Delete, a[i]);
            i += 1;
        } else if j < b.len() && inserted.contains(&j) {
            push(DiffSegmentKind::Insert, b[j]);
            j += 1;
        } else if i < a.len() && j < b.len() {
            push(DiffSegmentKind::Equal, a[i]);
            i += 1;
            j += 1;
        } else if i < a.len() {
            push(DiffSegmentKind::Delete, a[i]);
            i += 1;
        } else {
            push(DiffSegmentKind::Insert, b[j]);
            j += 1;
        }
    }
    merge_whitespace_only_equals(segments)
}

/// Absorbs single-space "equal" runs between changes so the diff reads as phrases, not confetti.
fn merge_whitespace_only_equals(segments: Vec<DiffSegment>) -> Vec<DiffSegment> {
    if segments.len() < 3 {
        return segments;
    }
    let mut out: Vec<DiffSegment> = Vec::new();
    let mut idx = 0;
    while idx < segments.len() {
        let s = &segments[idx];
        if s.kind == DiffSegmentKind::Equal
            && s.text.chars().all(|c| c == ' ')
            && !out.is_empty()
            && idx + 1 < segments.len()
            && out.last().is_some_and(|l| l.kind != DiffSegmentKind::Equal)
            && segments[idx + 1].kind != DiffSegmentKind::Equal
        {
            // Fold the space into both the deletion and the insertion runs.
            let mut dels = String::new();
            let mut ins = String::new();
            // Collect trailing changes already in `out`.
            while let Some(last) = out.last() {
                if last.kind == DiffSegmentKind::Equal {
                    break;
                }
                let last = out.pop().expect("checked");
                if last.kind == DiffSegmentKind::Delete {
                    dels = last.text + &dels;
                } else {
                    ins = last.text + &ins;
                }
            }
            dels.push_str(&s.text);
            ins.push_str(&s.text);
            let mut k = idx + 1;
            while k < segments.len() && segments[k].kind != DiffSegmentKind::Equal {
                if segments[k].kind == DiffSegmentKind::Delete {
                    dels.push_str(&segments[k].text);
                } else {
                    ins.push_str(&segments[k].text);
                }
                k += 1;
            }
            if !dels.is_empty() {
                out.push(DiffSegment::new(DiffSegmentKind::Delete, dels));
            }
            if !ins.is_empty() {
                out.push(DiffSegment::new(DiffSegmentKind::Insert, ins));
            }
            idx = k;
            continue;
        }
        out.push(s.clone());
        idx += 1;
    }
    out
}

/// Tracks a frozen text range through later edits so a returned proposal can be marked stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenScope {
    range: Range<usize>,
    original: String,
    is_stale: bool,
}

impl FrozenScope {
    pub fn new(range: Range<usize>, original: impl Into<String>) -> Self {
        FrozenScope {
            range,
            original: original.into(),
            is_stale: false,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn is_stale(&self) -> bool {
        self.is_stale
    }

    /// `edited_range` is the post-edit byte range of new text; `change_in_length` the byte length delta.
    pub fn note_edit(&mut self, edited_range: Range<usize>, change_in_length: isize) {
        if self.is_stale {
            return;
        }
        let pre_len = (edited_range.len() as isize - change_in_length).max(0) as usize;
        let pre = edited_range.start..edited_range.start + pre_len;
        if pre.end <= self.range.start {
            let start = (self.range.start as isize + change_in_length).max(0) as usize;
            self.range = start..start + self.range.len();
        } else if pre.start < self.range.end {
            // The edit touched the frozen range.
            self.is_stale = true;
        }
    }

    /// Confirms that the tracked range still holds the frozen text.
    pub fn verify(&mut self, text: &str) {
        if self.is_stale {
            return;
        }
        if text.get(self.range.clone()) != Some(self.original.as_str()) {
            self.is_stale = true;
        }
    }
}

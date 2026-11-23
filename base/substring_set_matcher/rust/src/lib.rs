// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::SubstringSetMatcher`
//! (base/substring_set_matcher/substring_set_matcher.{h,cc}).
//!
//! Given a fixed set of byte-string patterns, finds which patterns occur as
//! substrings of a text. Implemented as an Aho-Corasick automaton.
//!
//! Differences from the C++ version:
//! - `build` returns a new matcher or an error. The C++ `Build` mutates an
//!   object that must be built exactly once.
//! - Duplicate pattern strings are always ignored; the first one in input order
//!   wins. The C++ code CHECK-fails on them in DCHECK-enabled builds.
//! - Duplicate pattern IDs are allowed.
//! - Nodes are a plain `Vec` rather than the packed 12-byte layout, so memory
//!   use is higher. The ID and node-count limits are kept, so `build` fails in
//!   the same cases as the C++ code.
//! - `EstimateMemoryUsage` is not ported.

#![forbid(unsafe_code)]

use std::collections::{BTreeSet, VecDeque};

/// Index of a node in the automaton. Node 0 is the root.
type NodeId = usize;
const ROOT: NodeId = 0;

/// Same limit as the C++ `kInvalidNodeID`. Pattern IDs and node counts must
/// stay below it, because the C++ implementation packs them into 23 bits.
const MAX_NODE_ID: usize = (1 << 23) - 1;

/// A pattern to match, consisting of raw bytes and a caller-chosen ID.
///
/// Field order matters: the derived `Ord` compares `id` first, like the C++
/// `MatcherStringPattern::operator<`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatcherStringPattern {
    id: usize,
    pattern: Vec<u8>,
}

impl MatcherStringPattern {
    /// An ID that clients must not use.
    pub const INVALID_ID: usize = usize::MAX;

    /// Panics if `id` is `INVALID_ID`, matching the C++ `DCHECK_NE`.
    pub fn new(pattern: impl Into<Vec<u8>>, id: usize) -> Self {
        assert_ne!(id, Self::INVALID_ID, "pattern ID must not be INVALID_ID");
        Self { id, pattern: pattern.into() }
    }

    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

/// Reasons `SubstringSetMatcher::build` can fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildError {
    /// A pattern ID is not below the limit the automaton can store.
    IdTooLarge,
    /// The automaton would need too many nodes.
    TooManyNodes,
}

#[derive(Default)]
struct Node {
    /// Trie edges as `(byte, child)`. Searched linearly, which is fast for the
    /// small fan-out typical of most nodes.
    children: Vec<(u8, NodeId)>,
    /// Node for the longest proper suffix of this node's string that is also a
    /// trie node. `ROOT` if there is none.
    failure: NodeId,
    /// Nearest node along the failure chain that ends a pattern.
    output_link: Option<NodeId>,
    /// ID of the pattern that ends exactly at this node, if any.
    match_id: Option<usize>,
}

impl Node {
    fn child(&self, byte: u8) -> Option<NodeId> {
        self.children.iter().find(|&&(label, _)| label == byte).map(|&(_, node)| node)
    }

    fn has_outputs(&self) -> bool {
        self.match_id.is_some() || self.output_link.is_some()
    }
}

/// Matches a set of byte-string patterns against texts.
pub struct SubstringSetMatcher {
    nodes: Vec<Node>,
    is_empty: bool,
}

impl Default for SubstringSetMatcher {
    fn default() -> Self {
        // An empty pattern set always builds successfully.
        Self::build(&[]).expect("building an empty matcher cannot fail")
    }
}

impl SubstringSetMatcher {
    /// Builds a matcher for `patterns`.
    ///
    /// Complexity is O(n log n + S), where n is the number of patterns and S
    /// is the sum of their lengths. The sort is what contributes the log factor.
    pub fn build(patterns: &[MatcherStringPattern]) -> Result<Self, BuildError> {
        if patterns.iter().any(|p| p.id >= MAX_NODE_ID) {
            return Err(BuildError::IdTooLarge);
        }

        // A stable sort keeps the input order among equal pattern strings, so
        // the first registered duplicate is the one inserted and kept.
        let mut sorted: Vec<&MatcherStringPattern> = patterns.iter().collect();
        sorted.sort_by(|a, b| a.pattern.cmp(&b.pattern));

        let mut nodes = vec![Node::default()];
        for pattern in sorted {
            insert(&mut nodes, pattern);
        }
        if nodes.len() >= MAX_NODE_ID {
            return Err(BuildError::TooManyNodes);
        }
        link(&mut nodes);

        Ok(Self { nodes, is_empty: patterns.is_empty() })
    }

    /// Returns true if the matcher was built from an empty pattern set.
    pub fn is_empty(&self) -> bool {
        self.is_empty
    }

    /// Adds the IDs of all patterns that occur in `text` to `matches`.
    ///
    /// `matches` is not cleared first. Returns true if it gained any IDs.
    /// Complexity is O(t + z log z), where t is the text length and z is the
    /// number of matches.
    pub fn match_text(&self, text: &[u8], matches: &mut BTreeSet<usize>) -> bool {
        let before = matches.len();
        // Patterns that match the empty string are reported at the root.
        self.accumulate(ROOT, matches);

        let mut state = ROOT;
        for &byte in text {
            let next = self.step(state, byte);
            // A child is never the root, so `ROOT` means no transition was
            // found and there is nothing new to report.
            if next != ROOT {
                self.accumulate(next, matches);
            }
            state = next;
        }
        matches.len() != before
    }

    /// Returns true if any pattern occurs in `text`. Stops at the first hit and
    /// does not allocate.
    pub fn any_match(&self, text: &[u8]) -> bool {
        if self.nodes[ROOT].has_outputs() {
            return true;
        }
        let mut state = ROOT;
        for &byte in text {
            state = self.step(state, byte);
            if state != ROOT && self.nodes[state].has_outputs() {
                return true;
            }
        }
        false
    }

    /// Goto function with failure transitions. Follows failure links until a
    /// child labelled `byte` exists, and returns `ROOT` if none does.
    fn step(&self, mut state: NodeId, byte: u8) -> NodeId {
        loop {
            if let Some(child) = self.nodes[state].child(byte) {
                return child;
            }
            if state == ROOT {
                return ROOT;
            }
            state = self.nodes[state].failure;
        }
    }

    /// Adds the pattern ending at `node`, and at every node on its output-link
    /// chain, to `matches`.
    fn accumulate(&self, node: NodeId, matches: &mut BTreeSet<usize>) {
        let current = &self.nodes[node];
        if !current.has_outputs() {
            return;
        }
        if let Some(id) = current.match_id {
            matches.insert(id);
        }
        let mut link = current.output_link;
        while let Some(linked) = link {
            let linked_node = &self.nodes[linked];
            // Output links only point at nodes that end a pattern.
            if let Some(id) = linked_node.match_id {
                matches.insert(id);
            }
            link = linked_node.output_link;
        }
    }
}

/// Inserts `pattern` into the trie, creating nodes as needed.
fn insert(nodes: &mut Vec<Node>, pattern: &MatcherStringPattern) {
    let mut current = ROOT;
    for &byte in &pattern.pattern {
        current = match nodes[current].child(byte) {
            Some(child) => child,
            None => {
                let child = nodes.len();
                nodes.push(Node::default());
                nodes[current].children.push((byte, child));
                child
            }
        };
    }
    // The first pattern with a given string wins. Later duplicates are ignored.
    if nodes[current].match_id.is_none() {
        nodes[current].match_id = Some(pattern.id);
    }
}

/// Computes failure and output links with a breadth-first traversal.
fn link(nodes: &mut [Node]) {
    // The root has no failure link. It links to itself only when it ends a
    // pattern, which is the empty pattern.
    let root_output = nodes[ROOT].match_id.map(|_| ROOT);

    let mut queue = VecDeque::new();
    let root_children: Vec<NodeId> = nodes[ROOT].children.iter().map(|&(_, c)| c).collect();
    for child in root_children {
        nodes[child].output_link = root_output;
        queue.push_back(child);
    }

    while let Some(current) = queue.pop_front() {
        let edges = nodes[current].children.clone();
        for (byte, child) in edges {
            // Find the longest proper suffix of `child`'s string that is also a
            // trie node, by walking the failure chain of `current`.
            let mut candidate_parent = nodes[current].failure;
            let mut candidate = nodes[candidate_parent].child(byte);
            while candidate.is_none() && candidate_parent != ROOT {
                candidate_parent = nodes[candidate_parent].failure;
                candidate = nodes[candidate_parent].child(byte);
            }
            let failure = candidate.unwrap_or(ROOT);

            nodes[child].failure = failure;
            nodes[child].output_link = if nodes[failure].match_id.is_some() {
                Some(failure)
            } else {
                nodes[failure].output_link
            };
            queue.push_back(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn build(patterns: &[(&str, usize)]) -> SubstringSetMatcher {
        let patterns: Vec<MatcherStringPattern> =
            patterns.iter().map(|&(p, id)| MatcherStringPattern::new(p, id)).collect();
        SubstringSetMatcher::build(&patterns).expect("build should succeed")
    }

    fn matches_of(matcher: &SubstringSetMatcher, text: impl AsRef<[u8]>) -> BTreeSet<usize> {
        let mut matches = BTreeSet::new();
        matcher.match_text(text.as_ref(), &mut matches);
        matches
    }

    /// Checks the matches in both registration orders, as the C++ tests do.
    fn check(text: &str, patterns: &[(&str, usize)], expected: &[usize]) {
        let expected: BTreeSet<usize> = expected.iter().copied().collect();
        let mut reversed = patterns.to_vec();
        reversed.reverse();
        for ordering in [patterns.to_vec(), reversed] {
            let matcher = build(&ordering);
            assert_eq!(matches_of(&matcher, text), expected, "text={text:?} patterns={ordering:?}");
            assert_eq!(
                matcher.any_match(text.as_bytes()),
                !expected.is_empty(),
                "any_match text={text:?} patterns={ordering:?}"
            );
        }
    }

    #[test]
    fn overlapping_patterns() {
        // abcde with "bc" and "cd".
        check("abcde", &[("bc", 1), ("cd", 2)], &[1, 2]);
    }

    #[test]
    fn subpatterns() {
        check("abcde", &[("bc", 1), ("b", 2)], &[1, 2]);
        check("abcde", &[("bc", 1), ("c", 2)], &[1, 2]);
    }

    #[test]
    fn identical_match() {
        check("abcde", &[("abcde", 1)], &[1]);
    }

    #[test]
    fn multiple_occurrences() {
        check("abcde", &[("a", 1)], &[1]);
        check("aaaaa", &[("a", 1)], &[1]);
    }

    #[test]
    fn matches_at_beginning_and_end() {
        check("abcde", &[("ab", 1), ("de", 2)], &[1, 2]);
    }

    #[test]
    fn non_match() {
        check("abcde", &[("fg", 1)], &[]);
    }

    #[test]
    fn empty_pattern_and_too_long_pattern() {
        check("abcde", &[("", 1), ("abcdef", 2)], &[1]);
    }

    #[test]
    fn three_single_byte_patterns() {
        let all = build(&[("a", 1), ("b", 2), ("c", 3)]);
        assert_eq!(matches_of(&all, "abd"), BTreeSet::from([1, 2]));

        let some = build(&[("a", 1), ("c", 3)]);
        assert_eq!(matches_of(&some, "abd"), BTreeSet::from([1]));

        let empty = SubstringSetMatcher::build(&[]).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn every_substring_of_a_text_matches() {
        let text = "abcde";
        let mut patterns = vec![MatcherStringPattern::new("", 0)];
        let mut id = 1;
        for i in 0..text.len() {
            for j in i..text.len() {
                patterns.push(MatcherStringPattern::new(&text[i..=j], id));
                id += 1;
            }
        }
        let matcher = SubstringSetMatcher::build(&patterns).unwrap();
        let expected: BTreeSet<usize> = patterns.iter().map(|p| p.id()).collect();
        assert_eq!(matches_of(&matcher, text), expected);
    }

    #[test]
    fn empty_matcher_matches_nothing() {
        let matcher = SubstringSetMatcher::build(&[]).unwrap();
        assert!(matcher.is_empty());
        assert!(matches_of(&matcher, "abd").is_empty());
        assert!(!matcher.any_match(b"abd"));
    }

    #[test]
    fn default_matcher_is_empty() {
        let matcher = SubstringSetMatcher::default();
        assert!(matcher.is_empty());
        assert!(matches_of(&matcher, "anything").is_empty());
    }

    #[test]
    fn match_text_reports_whether_matches_grew() {
        let matcher = build(&[("ab", 1)]);
        let mut matches = BTreeSet::new();
        assert!(matcher.match_text(b"xaby", &mut matches));
        assert!(!matcher.match_text(b"xaby", &mut matches));
        assert!(!matcher.match_text(b"zzz", &mut matches));
    }

    #[test]
    fn matches_accumulate_without_clearing() {
        let matcher = build(&[("a", 1), ("b", 2)]);
        let mut matches = BTreeSet::from([9]);
        matcher.match_text(b"ab", &mut matches);
        assert_eq!(matches, BTreeSet::from([1, 2, 9]));
    }

    #[test]
    fn many_edges_from_one_node() {
        // Node "a" gets 256 outgoing byte edges, plus a match on "a" itself.
        let mut patterns = Vec::new();
        for i in 0..=255u8 {
            patterns.push(MatcherStringPattern::new(vec![b'a', i], i as usize));
        }
        patterns.push(MatcherStringPattern::new("a", 256));
        let matcher = SubstringSetMatcher::build(&patterns).unwrap();
        assert_eq!(matches_of(&matcher, "a\x07"), BTreeSet::from([7, 256]));
        assert_eq!(matches_of(&matcher, b"a\xff"), BTreeSet::from([255, 256]));
    }

    #[test]
    fn duplicate_pattern_strings_keep_the_first_registration() {
        // "a" gives "ba" a non-root failure link. Four duplicate "ba" patterns
        // follow, and 256 children hang off "ba".
        let mut patterns = vec![MatcherStringPattern::new("a", 0)];
        for id in 1..=4 {
            patterns.push(MatcherStringPattern::new("ba", id));
        }
        for i in 0..=255u8 {
            patterns.push(MatcherStringPattern::new(vec![b'b', b'a', i], 5 + i as usize));
        }
        let matcher = SubstringSetMatcher::build(&patterns).unwrap();
        assert_eq!(matches_of(&matcher, "ba"), BTreeSet::from([0, 1]));
    }

    #[test]
    fn duplicate_ids_are_reported_for_each_pattern() {
        let matcher = build(&[("x", 7), ("y", 7)]);
        assert_eq!(matches_of(&matcher, "xy"), BTreeSet::from([7]));
    }

    #[test]
    fn id_at_limit_is_rejected() {
        let ok = [MatcherStringPattern::new("a", MAX_NODE_ID - 1)];
        assert!(SubstringSetMatcher::build(&ok).is_ok());

        let too_large = [MatcherStringPattern::new("a", MAX_NODE_ID)];
        assert_eq!(SubstringSetMatcher::build(&too_large).err(), Some(BuildError::IdTooLarge));
    }

    #[test]
    #[should_panic(expected = "INVALID_ID")]
    fn invalid_id_panics_on_construction() {
        let _ = MatcherStringPattern::new("a", MatcherStringPattern::INVALID_ID);
    }

    #[test]
    fn pattern_ordering_compares_id_first() {
        let a = MatcherStringPattern::new("z", 1);
        let b = MatcherStringPattern::new("a", 2);
        assert!(a < b);
    }

    /// Small deterministic PRNG (xorshift64) so the tests need no dependencies.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn contains(text: &[u8], pattern: &[u8]) -> bool {
        pattern.is_empty() || text.windows(pattern.len()).any(|w| w == pattern)
    }

    #[test]
    fn agrees_with_brute_force_on_random_inputs() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..3000 {
            // Small alphabet so that overlaps, prefixes, and duplicates are common.
            let alphabet = *b"abc";
            let num_patterns = rng.below(12) as usize;
            let mut patterns = Vec::new();
            for id in 0..num_patterns {
                let len = rng.below(6) as usize;
                let bytes: Vec<u8> = (0..len).map(|_| alphabet[rng.below(3) as usize]).collect();
                patterns.push(MatcherStringPattern::new(bytes, id));
            }

            let text_len = rng.below(25) as usize;
            let text: Vec<u8> = (0..text_len).map(|_| alphabet[rng.below(3) as usize]).collect();

            // Reference: the first registered pattern for each string wins.
            let mut first_id_for_string: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
            for p in &patterns {
                first_id_for_string.entry(p.pattern().to_vec()).or_insert(p.id());
            }
            let expected: BTreeSet<usize> = first_id_for_string
                .iter()
                .filter(|(s, _)| contains(&text, s))
                .map(|(_, &id)| id)
                .collect();

            let matcher = SubstringSetMatcher::build(&patterns).unwrap();
            let mut actual = BTreeSet::new();
            matcher.match_text(&text, &mut actual);
            assert_eq!(
                actual,
                expected,
                "patterns={:?} text={:?}",
                patterns
                    .iter()
                    .map(|p| (String::from_utf8_lossy(p.pattern()).into_owned(), p.id()))
                    .collect::<Vec<_>>(),
                String::from_utf8_lossy(&text)
            );
            assert_eq!(matcher.any_match(&text), !expected.is_empty());
        }
    }
}

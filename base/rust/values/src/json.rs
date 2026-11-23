// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! The part of `base::JSONWriter` that `Value::DebugString()` uses
//! (base/json/json_writer.{h,cc} and base/json/string_escape.cc), with
//! `OPTIONS_PRETTY_PRINT` and the default `max_depth`.
//!
//! Output quirks kept from the C++ code:
//! - Output ends with `"\n"`, even for scalar values.
//! - A dictionary is `{`, a newline, then entries indented by 3 spaces per
//!   level, then a newline and `}`. An empty dictionary is `{\n}`.
//! - A list is `[ ` then the items separated by `, `, then ` ]`. An empty list
//!   is `[  ]`.
//! - Items in a list take the list's own indent depth, not one level more.
//! - A binary blob writes nothing (the key is still written, so `"k": ` is
//!   followed by the next separator). The blob makes the writer return false,
//!   but `DebugString()` ignores that.
//! - Containers nested deeper than `kAbsoluteMaxDepth` (200) write nothing.
//! - Doubles always show a `.` or an exponent, so `1.0` is `1.0`, not `1`.
//!   A leading `.` gets a `0` in front.
//! - Strings escape `"`, `\`, `<` (as `<`), U+2028, U+2029, and control
//!   characters below 0x20. Control characters use `\uXXXX` with uppercase hex,
//!   except for `\b \f \n \r \t`. Other characters are written as they are.
//!
//! Not ported: the other `JSONWriter` options (`OMIT_BINARY_VALUES`,
//! `OMIT_DOUBLE_TYPE_PRESERVATION`), the non-pretty output, and the `WriteJson`
//! helpers.

use std::fmt::Write as _;

use string_number_conversions::number_to_string;

use crate::dict::DictValue;
use crate::list::ListValue;
use crate::value::ValueView;

/// `json_internal::kAbsoluteMaxDepth`.
const ABSOLUTE_MAX_DEPTH: usize = 200;

/// `Value::DebugString()` / `DictValue::DebugString()` /
/// `ListValue::DebugString()`: pretty-printed JSON with a trailing newline.
pub(crate) fn debug_string(node: ValueView<'_>) -> String {
    let mut out = String::new();
    let mut writer = Writer { out: &mut out };
    // `max_depth` counts the top-level value, so subtract one here.
    writer.build(node, 0, ABSOLUTE_MAX_DEPTH - 1);
    out.push('\n');
    out
}

struct Writer<'o> {
    out: &'o mut String,
}

impl Writer<'_> {
    /// Writes `node`. Returns `false` if anything could not be written, which
    /// is the `BuildJSONString` result. Output is still written for the parts
    /// that succeed.
    fn build(&mut self, node: ValueView<'_>, indent_depth: usize, max_depth: usize) -> bool {
        match node {
            ValueView::None => {
                self.out.push_str("null");
                true
            }
            ValueView::Bool(b) => {
                self.out.push_str(if b { "true" } else { "false" });
                true
            }
            ValueView::Int(i) => {
                self.out.push_str(&number_to_string(i));
                true
            }
            ValueView::Double(d) => {
                self.write_double(d);
                true
            }
            ValueView::String(s) => {
                escape_json_string(s, true, self.out);
                true
            }
            // Binary data is not allowed, and nothing is written.
            ValueView::Blob(_) => false,
            ValueView::Dict(dict) => self.build_dict(dict, indent_depth, max_depth),
            ValueView::List(list) => self.build_list(list, indent_depth, max_depth),
        }
    }

    fn build_dict(&mut self, dict: &DictValue, indent_depth: usize, max_depth: usize) -> bool {
        if max_depth == 0 {
            return false;
        }
        let max_depth = max_depth - 1;

        self.out.push('{');
        self.out.push('\n');

        let mut first_value_has_been_output = false;
        let mut result = true;
        for (key, value) in dict.iter() {
            if first_value_has_been_output {
                self.out.push(',');
                self.out.push('\n');
            }

            self.indent(indent_depth + 1);
            escape_json_string(key, true, self.out);
            self.out.push(':');
            self.out.push(' ');

            let ok = self.build(value.view(), indent_depth + 1, max_depth);
            result &= ok;
            first_value_has_been_output = true;
        }

        if first_value_has_been_output {
            self.out.push('\n');
        }
        self.indent(indent_depth);
        self.out.push('}');
        result
    }

    fn build_list(&mut self, list: &ListValue, indent_depth: usize, max_depth: usize) -> bool {
        if max_depth == 0 {
            return false;
        }
        let max_depth = max_depth - 1;

        self.out.push('[');
        self.out.push(' ');

        let mut first_value_has_been_output = false;
        let mut result = true;
        for value in list.iter() {
            if first_value_has_been_output {
                self.out.push(',');
                self.out.push(' ');
            }

            // List items use the list's depth, as the C++ code does.
            let ok = self.build(value.view(), indent_depth, max_depth);
            result &= ok;
            first_value_has_been_output = true;
        }

        self.out.push(' ');
        self.out.push(']');
        result
    }

    /// `WriteDouble`-equivalent: shortest round-trip digits, with `.0` added
    /// for integral values and a leading `0` added before a bare fraction.
    fn write_double(&mut self, value: f64) {
        let mut real = number_to_string(value);
        if !real.contains(&['.', 'e', 'E'][..]) {
            real.push_str(".0");
        }
        if real.starts_with('.') {
            real.insert(0, '0');
        } else if real.len() > 1 && real.starts_with("-.") {
            real.insert(1, '0');
        }
        self.out.push_str(&real);
    }

    fn indent(&mut self, depth: usize) {
        for _ in 0..depth * 3 {
            self.out.push(' ');
        }
    }
}

/// `EscapeJSONString` with `put_in_quotes`. Appends the escaped string to
/// `out`.
pub(crate) fn escape_json_string(s: &str, put_in_quotes: bool, out: &mut String) {
    if put_in_quotes {
        out.push('"');
    }
    for c in s.chars() {
        match c {
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            // Escape `<` so the output is safe inside a `<script>`. `>` is not
            // escaped, as in C++.
            '<' => out.push_str("\\u003C"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    if put_in_quotes {
        out.push('"');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn scalars_end_with_newline() {
        assert_eq!(Value::from(false).debug_string(), "false\n");
        assert_eq!(Value::from(true).debug_string(), "true\n");
        assert_eq!(Value::from(-37).debug_string(), "-37\n");
        assert_eq!(Value::default().debug_string(), "null\n");
    }

    #[test]
    fn doubles_always_have_a_fraction_or_exponent() {
        assert_eq!(Value::from(1.0).debug_string(), "1.0\n");
        assert_eq!(Value::from(-4.655).debug_string(), "-4.655\n");
        assert_eq!(Value::from(0.5).debug_string(), "0.5\n");
        assert_eq!(Value::from(-0.5).debug_string(), "-0.5\n");
        assert_eq!(Value::from(1e20).debug_string(), "1e+20\n");
    }

    #[test]
    fn strings_are_escaped() {
        let mut out = String::new();
        escape_json_string("a\"b\\c\n\t\u{1}<>\u{2028}é", true, &mut out);
        assert_eq!(out, "\"a\\\"b\\\\c\\n\\t\\u0001\\u003C>\\u2028é\"");
    }

    #[test]
    fn empty_containers() {
        assert_eq!(DictValue::new().debug_string(), "{\n}\n");
        assert_eq!(ListValue::new().debug_string(), "[  ]\n");
    }

    #[test]
    fn nested_dict_layout() {
        let dict = DictValue::new()
            .with("a", 1)
            .with("b", ListValue::new().with(true).with("x"));
        assert_eq!(
            dict.debug_string(),
            "{\n   \"a\": 1,\n   \"b\": [ true, \"x\" ]\n}\n"
        );
    }

    #[test]
    fn list_items_use_list_depth() {
        // A dictionary inside a list keeps the depth of the list, so its
        // entries are indented one level more than the list itself.
        let inner = DictValue::new().with("k", 2);
        let dict = DictValue::new().with("list", ListValue::new().with(inner));
        assert_eq!(
            dict.debug_string(),
            "{\n   \"list\": [ {\n      \"k\": 2\n   } ]\n}\n"
        );
    }

    #[test]
    fn blob_writes_nothing_after_its_key() {
        let dict = DictValue::new()
            .with("blob", vec![1u8, 2])
            .with("n", 1);
        assert_eq!(dict.debug_string(), "{\n   \"blob\": ,\n   \"n\": 1\n}\n");
        assert_eq!(Value::from(vec![1u8]).debug_string(), "\n");
    }

    #[test]
    fn depth_limit_drops_deep_containers() {
        // 199 nested lists plus the top-level value fit exactly. The next one
        // writes nothing.
        let mut value = Value::from(ListValue::new());
        for _ in 0..200 {
            value = Value::from(ListValue::new().with(value));
        }
        let text = value.debug_string();
        assert!(text.starts_with("[ [ "));
        assert!(text.ends_with("]\n"));
    }
}

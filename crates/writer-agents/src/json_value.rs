//! Minimal typed JSON for the line-delimited protocols the writing agents speak (Swift `JSONValue`).
//!
//! Numbers are `f64`, as in Swift. Objects keep their keys sorted, so [`JsonValue::serialized`]
//! writes keys in order the way Swift's `JSONEncoder` with `.sortedKeys` did.

use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum JsonValue {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    /// `value[key]` for an object; `None` for anything else or a missing key.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(o) => o.get(key),
            _ => None,
        }
    }

    /// `value[a]?[b]?…`
    pub fn at(&self, path: &[&str]) -> Option<&JsonValue> {
        path.iter().try_fold(self, |v, k| v.get(k))
    }

    pub fn string_value(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn number_value(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Parses one JSON document. `None` when it isn't valid JSON.
    pub fn parse(line: &str) -> Option<JsonValue> {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .map(JsonValue::from)
    }

    /// Compact JSON with sorted keys and unescaped slashes. `"null"` when the value can't be
    /// encoded (a NaN or infinite number), as the Swift encoder failed then.
    pub fn serialized(&self) -> String {
        let mut out = String::new();
        if write_value(&mut out, self, None) {
            out
        } else {
            "null".to_string()
        }
    }

    /// Pretty-printed like `JSONEncoder` with `[.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]`:
    /// two-space indent and `"key" : value`. Falls back to [`JsonValue::serialized`].
    pub fn pretty_printed(&self) -> String {
        let mut out = String::new();
        if write_value(&mut out, self, Some(0)) {
            out
        } else {
            self.serialized()
        }
    }

    /// An object from `(key, value)` pairs; later duplicates win.
    pub fn object<K: Into<String>>(pairs: impl IntoIterator<Item = (K, JsonValue)>) -> JsonValue {
        JsonValue::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }
}

fn write_value(out: &mut String, v: &JsonValue, indent: Option<usize>) -> bool {
    match v {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        JsonValue::Number(n) => {
            if !n.is_finite() {
                return false;
            }
            if n.round() == *n && n.abs() < 1e15 {
                let _ = write!(out, "{}", *n as i64);
            } else {
                // Not reached by the protocols here; ryu's shortest form, which can differ from
                // Swift's `Double.description` in exponent spelling.
                out.push_str(&serde_json::Number::from_f64(*n).map_or("null".into(), |x| x.to_string()));
            }
        }
        JsonValue::String(s) => write_string(out, s),
        JsonValue::Array(a) => {
            out.push('[');
            if let Some(level) = indent {
                out.push('\n');
                for (i, e) in a.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    push_indent(out, level + 1);
                    if !write_value(out, e, Some(level + 1)) {
                        return false;
                    }
                }
                out.push('\n');
                push_indent(out, level);
            } else {
                for (i, e) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    if !write_value(out, e, None) {
                        return false;
                    }
                }
            }
            out.push(']');
        }
        JsonValue::Object(o) => {
            out.push('{');
            if let Some(level) = indent {
                out.push('\n');
                for (i, (k, e)) in o.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    push_indent(out, level + 1);
                    write_string(out, k);
                    out.push_str(" : ");
                    if !write_value(out, e, Some(level + 1)) {
                        return false;
                    }
                }
                out.push('\n');
                push_indent(out, level);
            } else {
                for (i, (k, e)) in o.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, k);
                    out.push(':');
                    if !write_value(out, e, None) {
                        return false;
                    }
                }
            }
            out.push('}');
        }
    }
    true
}

fn push_indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl From<serde_json::Value> for JsonValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => JsonValue::Null,
            serde_json::Value::Bool(b) => JsonValue::Bool(b),
            serde_json::Value::Number(n) => JsonValue::Number(n.as_f64().unwrap_or(f64::NAN)),
            serde_json::Value::String(s) => JsonValue::String(s),
            serde_json::Value::Array(a) => JsonValue::Array(a.into_iter().map(Into::into).collect()),
            serde_json::Value::Object(o) => {
                JsonValue::Object(o.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

impl From<&str> for JsonValue {
    fn from(s: &str) -> Self {
        JsonValue::String(s.to_string())
    }
}

impl From<String> for JsonValue {
    fn from(s: String) -> Self {
        JsonValue::String(s)
    }
}

impl From<bool> for JsonValue {
    fn from(b: bool) -> Self {
        JsonValue::Bool(b)
    }
}

impl From<f64> for JsonValue {
    fn from(n: f64) -> Self {
        JsonValue::Number(n)
    }
}

impl From<i64> for JsonValue {
    fn from(n: i64) -> Self {
        JsonValue::Number(n as f64)
    }
}

impl From<usize> for JsonValue {
    fn from(n: usize) -> Self {
        JsonValue::Number(n as f64)
    }
}

impl From<Vec<JsonValue>> for JsonValue {
    fn from(a: Vec<JsonValue>) -> Self {
        JsonValue::Array(a)
    }
}

impl PartialEq<&str> for JsonValue {
    fn eq(&self, other: &&str) -> bool {
        self.string_value() == Some(*other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorted_compact_and_pretty() {
        let v: JsonValue = json!({"b": [1, 2.5, "a/b\n"], "a": {}, "c": []}).into();
        assert_eq!(v.serialized(), r#"{"a":{},"b":[1,2.5,"a/b\n"],"c":[]}"#);
        assert_eq!(
            v.pretty_printed(),
            "{\n  \"a\" : {\n\n  },\n  \"b\" : [\n    1,\n    2.5,\n    \"a/b\\n\"\n  ],\n  \"c\" : [\n\n  ]\n}"
        );
        assert_eq!(JsonValue::Number(f64::NAN).serialized(), "null");
        assert_eq!(JsonValue::from("\u{1}").serialized(), "\"\\u0001\"");
    }
}

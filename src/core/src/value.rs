use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Canonical result representation shared by all DB packages.
///
/// Every DB package coerces its driver-native results into this enum, so
/// that all cross-type comparison rules live in exactly one place:
/// [`Value::matches_expected`]. The derived `==` is strict structural
/// equality (`Int(3) != Float(3.0)`) — use it in tests, never for scoring
/// benchmark accuracy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, from = "serde_json::Value")]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// Compare a coerced query result against a question's expected value.
    /// This is the benchmark's accuracy rule — distinct from `==`.
    ///
    /// Every cross-type rule here is a deliberate decision; combinations not
    /// listed are unequal by design.
    pub fn matches_expected(&self, expected: &Value) -> bool {
        match (self, expected) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            // Compared in the integer domain: the float must be a whole
            // number strictly inside i64 range (so the cast below is exact,
            // never saturating) whose value is `a`. Int-to-float casts can't
            // be trusted here — they round at the extremes.
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => {
                *b >= -2f64.powi(63) && *b < 2f64.powi(63) && b.fract() == 0.0 && *b as i64 == *a
            }
            (Value::String(a), Value::String(b)) => a == b,
            // Ordered: at this depth a list is a row/tuple whose order is
            // meaningful. Top-level row-order semantics are handled by
            // [`Value::matches_question`].
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.matches_expected(y))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|w| v.matches_expected(w)))
            }
            // A one-field object where a bare value was expected is that
            // value: the field name is a label the model chose, not part of
            // the answer (TypeDB `fetch { "count": $n }`, Cypher `RETURN
            // {n: ...}`). Applies per element inside lists too, via the
            // list rules above. When the expected value is itself an object
            // the field names are part of the shape and are not unwrapped.
            (Value::Object(a), _) if a.len() == 1 => {
                a.values().next().is_some_and(|v| v.matches_expected(expected))
            }
            _ => false,
        }
    }

    /// Top-level comparison for a question's result, honouring the
    /// question's `ordered` flag. When `ordered` is false and both sides are
    /// lists, the top-level list compares as a bag (DBs may return rows in
    /// any order); nested lists always compare ordered, as tuples.
    pub fn matches_question(&self, expected: &Value, ordered: bool) -> bool {
        match (self, expected) {
            (Value::List(result), Value::List(expected)) if !ordered => {
                bag_matches(result, expected)
            }
            _ => self.matches_expected(expected),
        }
    }
}

/// Shape a table of query results into a canonical [`Value`] — the shared
/// rule for all DB packages so cross-DB comparisons stay fair:
/// - multiple columns: each row becomes an object keyed by column name
/// - one column: each row becomes the bare value
/// - a single row is unwrapped (a lone count compares against a scalar)
/// - no rows: an empty list
pub fn shape_rows(columns: &[String], rows: Vec<Vec<Value>>) -> Value {
    let mut shaped: Vec<Value> = rows
        .into_iter()
        .map(|row| shape_row(columns, row))
        .collect();
    if shaped.len() == 1 {
        shaped.pop().unwrap()
    } else {
        Value::List(shaped)
    }
}

fn shape_row(columns: &[String], mut row: Vec<Value>) -> Value {
    if columns.len() == 1 {
        row.remove(0)
    } else {
        Value::Object(columns.iter().cloned().zip(row).collect())
    }
}

/// Multiset comparison: every result row consumes exactly one expected row.
/// Matching is greedy, which suffices because cross-type equality only
/// crosses Int/Float and that rule is symmetric.
fn bag_matches(result: &[Value], expected: &[Value]) -> bool {
    if result.len() != expected.len() {
        return false;
    }
    let mut remaining: Vec<&Value> = expected.iter().collect();
    for row in result {
        match remaining.iter().position(|e| row.matches_expected(e)) {
            Some(i) => {
                remaining.swap_remove(i);
            }
            None => return false,
        }
    }
    true
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => match n.as_i64() {
                Some(i) => Value::Int(i),
                None => Value::Float(n.as_f64().unwrap_or(f64::NAN)),
            },
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(a) => Value::List(a.into_iter().map(Value::from).collect()),
            serde_json::Value::Object(o) => {
                Value::Object(o.into_iter().map(|(k, v)| (k, Value::from(v))).collect())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_and_float_match_when_the_int_round_trips() {
        assert!(Value::Int(3).matches_expected(&Value::Float(3.0)));
        assert!(Value::Float(3.0).matches_expected(&Value::Int(3)));
        assert!(!Value::Int(3).matches_expected(&Value::Float(3.5)));
        // i64::MAX is not representable in f64; the cast rounds, so this
        // must not compare equal.
        assert!(!Value::Int(i64::MAX).matches_expected(&Value::Float(i64::MAX as f64)));
    }

    #[test]
    fn structural_equality_stays_strict() {
        assert_ne!(Value::Int(3), Value::Float(3.0));
        assert!(!Value::String("3".into()).matches_expected(&Value::Int(3)));
        assert!(!Value::Bool(true).matches_expected(&Value::Int(1)));
    }

    fn ints(values: &[i64]) -> Value {
        Value::List(values.iter().copied().map(Value::Int).collect())
    }

    #[test]
    fn unordered_lists_compare_as_bags() {
        assert!(ints(&[1, 2, 3]).matches_question(&ints(&[3, 1, 2]), false));
        // Multiplicity counts: a bag isn't a set.
        assert!(!ints(&[1, 1, 2]).matches_question(&ints(&[1, 2, 2]), false));
        assert!(!ints(&[1, 2]).matches_question(&ints(&[1, 2, 3]), false));
    }

    #[test]
    fn ordered_lists_reject_reordering() {
        assert!(ints(&[1, 2, 3]).matches_question(&ints(&[1, 2, 3]), true));
        assert!(!ints(&[1, 2, 3]).matches_question(&ints(&[3, 1, 2]), true));
    }

    #[test]
    fn shaping_unwraps_single_column_and_single_row() {
        let col = vec!["count".to_string()];
        assert_eq!(shape_rows(&col, vec![vec![Value::Int(3)]]), Value::Int(3));
        assert_eq!(
            shape_rows(&col, vec![vec![Value::Int(1)], vec![Value::Int(2)]]),
            ints(&[1, 2])
        );
        assert_eq!(shape_rows(&col, vec![]), Value::List(vec![]));

        let cols = vec!["name".to_string(), "age".to_string()];
        let shaped = shape_rows(&cols, vec![vec![Value::String("ka".into()), Value::Int(2)]]);
        assert_eq!(
            shaped,
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("ka".into())),
                ("age".to_string(), Value::Int(2)),
            ]))
        );
    }

    #[test]
    fn one_field_object_unwraps_to_its_value() {
        let doc = |k: &str, v: Value| Value::Object(BTreeMap::from([(k.to_string(), v)]));
        assert!(doc("pct", Value::Float(62.4)).matches_expected(&Value::Float(62.4)));
        assert!(doc("n", Value::Int(3)).matches_expected(&Value::Float(3.0)));
        assert!(!doc("n", Value::Int(4)).matches_expected(&Value::Int(3)));
        // Per element inside a list, in bag mode too.
        let names = Value::List(vec![doc("a", Value::String("x".into())), doc("b", Value::String("y".into()))]);
        let expected = Value::List(vec![Value::String("y".into()), Value::String("x".into())]);
        assert!(names.matches_question(&expected, false));
        // Only one field unwraps; and an expected object keeps its field names.
        let two = Value::Object(BTreeMap::from([
            ("a".to_string(), Value::Int(1)),
            ("b".to_string(), Value::Int(2)),
        ]));
        assert!(!two.matches_expected(&Value::Int(1)));
        assert!(!doc("wrong", Value::Int(1)).matches_expected(&doc("right", Value::Int(1))));
    }

    #[test]
    fn nested_lists_stay_ordered_even_in_bag_mode() {
        let result = Value::List(vec![ints(&[1, 2]), ints(&[3, 4])]);
        let reordered_rows = Value::List(vec![ints(&[3, 4]), ints(&[1, 2])]);
        let reordered_tuple = Value::List(vec![ints(&[2, 1]), ints(&[3, 4])]);
        assert!(result.matches_question(&reordered_rows, false));
        assert!(!result.matches_question(&reordered_tuple, false));
    }
}

//! Apply validation's certain fixes to a scene, round after round, until none
//! remain (`AAA-AI-03`).
//!
//! Validation says what is wrong and how to fix it. Where the fix needs no
//! judgement — a misspelled property or id with exactly one near match — it
//! also says so as a JSON Patch, `ValidationError::fix_patch`. This applies
//! those patches, validates again, and repeats, because one repair can expose
//! the next: a timeline entry naming a misspelled object has its properties
//! checked only once it names a real one.
//!
//! What it will not do is guess. A wrong type, a value out of range, or a name
//! with no near match stays in [`FixReport::remaining`] for a person or a model
//! to decide.

use serde::Serialize;
use serde_json::Value;

use crate::validation::{validate_scene_json, ValidationResponse};

/// How many rounds [`fix_scene`] runs before giving up, when asked for the
/// default. Each round can only turn invalid names into valid ones, so a real
/// scene settles in two or three; the bound is a backstop, not a budget.
pub const DEFAULT_MAX_ROUNDS: usize = 8;

/// One fix [`fix_scene`] applied.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedFix {
    /// The error code the fix answers, e.g. `UNKNOWN_PROPERTY`.
    pub code: String,
    /// Where the error was, as validation reported it.
    pub path: String,
    /// The fix in words, as validation gave it.
    pub fix_suggestion: String,
    /// The JSON Patch that was applied.
    pub patch: Vec<Value>,
    /// The round it was applied in, from 1.
    pub round: usize,
}

/// What [`fix_scene`] changed, and what is still wrong.
#[derive(Debug, Clone, Serialize)]
pub struct FixReport {
    /// The scene with every applied fix in it.
    pub scene: Value,
    /// Each fix, in the order applied.
    pub applied: Vec<AppliedFix>,
    /// Validation of the fixed scene: what needs a decision.
    pub remaining: ValidationResponse,
}

/// Apply every certain fix to `raw`, re-validating between rounds, for at most
/// `max_rounds` rounds.
#[must_use]
pub fn fix_scene(raw: &Value, max_rounds: usize) -> FixReport {
    let mut scene = raw.clone();
    let mut applied = Vec::new();
    for round in 1..=max_rounds {
        let mut changed = false;
        for error in validate_scene_json(&scene).errors {
            let Some(patch) = error.fix_patch else {
                continue;
            };
            // A patch computed against the start of the round can be stale by
            // the time it is reached — two misspellings of one property both
            // renamed to it, say. It is skipped, and the next round
            // re-validates and says what is left.
            if apply_patch(&mut scene, &patch).is_ok() {
                changed = true;
                applied.push(AppliedFix {
                    code: error.code,
                    path: error.path,
                    fix_suggestion: error.fix_suggestion,
                    patch,
                    round,
                });
            }
        }
        if !changed {
            break;
        }
    }
    let remaining = validate_scene_json(&scene);
    FixReport {
        scene,
        applied,
        remaining,
    }
}

/// Why a patch could not be applied.
#[derive(Debug, PartialEq, Eq)]
pub enum PatchError {
    /// An operation other than the `replace` and `move` validation emits.
    UnsupportedOperation(String),
    /// A pointer that does not lead anywhere in the document.
    NoSuchPath(String),
    /// A `move` onto a key that already holds a value.
    WouldOverwrite(String),
}

/// Apply a patch made of the operations `fix_patch` contains: `replace`, and
/// `move` between keys of JSON objects.
///
/// All or nothing: the document is unchanged unless every operation applies.
/// Stricter than RFC 6902 in one place — a `move` onto an existing key is
/// refused rather than overwriting it, so two misspellings of one property can
/// never quietly collapse into a single value.
///
/// # Errors
///
/// Returns the first operation that could not be applied.
pub fn apply_patch(doc: &mut Value, patch: &[Value]) -> Result<(), PatchError> {
    let mut next = doc.clone();
    for op in patch {
        let path = op["path"].as_str().unwrap_or_default();
        match op["op"].as_str().unwrap_or_default() {
            "replace" => {
                let target = next
                    .pointer_mut(path)
                    .ok_or_else(|| PatchError::NoSuchPath(path.to_string()))?;
                *target = op["value"].clone();
            }
            "move" => {
                let from = op["from"].as_str().unwrap_or_default();
                let value = take(&mut next, from)?;
                let (parent, key) =
                    split(path).ok_or_else(|| PatchError::NoSuchPath(path.to_string()))?;
                let map = next
                    .pointer_mut(parent)
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| PatchError::NoSuchPath(path.to_string()))?;
                if map.contains_key(&key) {
                    return Err(PatchError::WouldOverwrite(path.to_string()));
                }
                map.insert(key, value);
            }
            other => return Err(PatchError::UnsupportedOperation(other.to_string())),
        }
    }
    *doc = next;
    Ok(())
}

/// Remove and return the value at an object key `pointer` names.
fn take(doc: &mut Value, pointer: &str) -> Result<Value, PatchError> {
    let missing = || PatchError::NoSuchPath(pointer.to_string());
    let (parent, key) = split(pointer).ok_or_else(missing)?;
    doc.pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .and_then(|map| map.remove(&key))
        .ok_or_else(missing)
}

/// Split a JSON Pointer into its parent pointer and its unescaped last key.
fn split(pointer: &str) -> Option<(&str, String)> {
    let at = pointer.rfind('/')?;
    let key = pointer[at + 1..].replace("~1", "/").replace("~0", "~");
    Some((&pointer[..at], key))
}

/// What [`fix_text`] changed in a scene file, and what is still wrong.
#[derive(Debug, Clone, Serialize)]
pub struct TextFixReport {
    /// The file with every applied fix in it, otherwise byte for byte as it was.
    pub text: String,
    /// Each fix, in the order applied.
    pub applied: Vec<AppliedFix>,
    /// Validation of the fixed scene: what needs a decision.
    pub remaining: ValidationResponse,
}

/// [`fix_scene`] for a scene file's text, changing only the tokens each fix
/// replaces.
///
/// Going through a parsed document would rewrite the whole file: key order
/// would come back sorted and every line reformatted, so a one-word typo fix
/// became a diff of the entire scene. Here a renamed property keeps its place
/// and the file keeps its layout; the diff is the corrected words.
///
/// # Errors
///
/// Returns an error if `text` is not JSON.
pub fn fix_text(text: &str, max_rounds: usize) -> Result<TextFixReport, serde_json::Error> {
    let mut text = text.to_string();
    let mut applied = Vec::new();
    for round in 1..=max_rounds {
        let doc: Value = serde_json::from_str(&text)?;
        let mut changed = false;
        for error in validate_scene_json(&doc).errors {
            let Some(patch) = error.fix_patch else {
                continue;
            };
            if let Ok(next) = patch_text(&text, &patch) {
                text = next;
                changed = true;
                applied.push(AppliedFix {
                    code: error.code,
                    path: error.path,
                    fix_suggestion: error.fix_suggestion,
                    patch,
                    round,
                });
            }
        }
        if !changed {
            break;
        }
    }
    let remaining = validate_scene_json(&serde_json::from_str(&text)?);
    Ok(TextFixReport {
        text,
        applied,
        remaining,
    })
}

/// [`apply_patch`] on a JSON document's text: each operation replaces exactly
/// one token, so everything else in the text is kept.
///
/// Supports what `fix_patch` contains — `replace`, and `move` between two keys
/// of the same object, which is a rename in place.
///
/// # Errors
///
/// As [`apply_patch`], plus [`PatchError::UnsupportedOperation`] for a `move`
/// between different objects, which cannot be done by replacing one token.
pub fn patch_text(text: &str, patch: &[Value]) -> Result<String, PatchError> {
    let mut text = text.to_string();
    for op in patch {
        let path = op["path"].as_str().unwrap_or_default();
        let missing = || PatchError::NoSuchPath(path.to_string());
        let (range, replacement) = match op["op"].as_str().unwrap_or_default() {
            "replace" => {
                let at = locate(&text, path).ok_or_else(missing)?;
                (at.value, op["value"].to_string())
            }
            "move" => {
                let from = op["from"].as_str().unwrap_or_default();
                let (from_parent, _) =
                    split(from).ok_or_else(|| PatchError::NoSuchPath(from.to_string()))?;
                let (parent, key) = split(path).ok_or_else(missing)?;
                if from_parent != parent {
                    return Err(PatchError::UnsupportedOperation(
                        "move between objects".to_string(),
                    ));
                }
                if locate(&text, path).is_some() {
                    return Err(PatchError::WouldOverwrite(path.to_string()));
                }
                let at =
                    locate(&text, from).ok_or_else(|| PatchError::NoSuchPath(from.to_string()))?;
                let key_span = at.key.ok_or_else(missing)?;
                (key_span, Value::String(key).to_string())
            }
            other => return Err(PatchError::UnsupportedOperation(other.to_string())),
        };
        text.replace_range(range, &replacement);
    }
    Ok(text)
}

/// Where a pointer's value, and its key if it has one, sit in a document.
struct Located {
    key: Option<std::ops::Range<usize>>,
    value: std::ops::Range<usize>,
}

/// Find the tokens a JSON Pointer names, by walking the text rather than
/// parsing it into a tree.
fn locate(text: &str, pointer: &str) -> Option<Located> {
    let bytes = text.as_bytes();
    let mut at = Located {
        key: None,
        value: skip_ws(bytes, 0)..0,
    };
    at.value.end = value_end(bytes, at.value.start)?;
    if pointer.is_empty() {
        return Some(at);
    }
    for raw in pointer.strip_prefix('/')?.split('/') {
        let segment = raw.replace("~1", "/").replace("~0", "~");
        let start = at.value.start;
        at = match bytes.get(start)? {
            b'{' => member(text, start, &segment)?,
            b'[' => element(bytes, start, segment.parse().ok()?)?,
            _ => return None,
        };
    }
    Some(at)
}

/// The member of the object starting at `start` whose key is `wanted`.
fn member(text: &str, start: usize, wanted: &str) -> Option<Located> {
    let bytes = text.as_bytes();
    let mut i = skip_ws(bytes, start + 1);
    if bytes.get(i) == Some(&b'}') {
        return None;
    }
    loop {
        let key_end = string_end(bytes, i)?;
        // Decoded, so a key written with escapes still matches its name.
        let key: String = serde_json::from_str(&text[i..key_end]).ok()?;
        let colon = skip_ws(bytes, key_end);
        if bytes.get(colon) != Some(&b':') {
            return None;
        }
        let value_start = skip_ws(bytes, colon + 1);
        let end = value_end(bytes, value_start)?;
        if key == wanted {
            return Some(Located {
                key: Some(i..key_end),
                value: value_start..end,
            });
        }
        i = skip_ws(bytes, end);
        match bytes.get(i)? {
            b',' => i = skip_ws(bytes, i + 1),
            _ => return None,
        }
    }
}

/// Element `index` of the array starting at `start`.
fn element(bytes: &[u8], start: usize, index: usize) -> Option<Located> {
    let mut i = skip_ws(bytes, start + 1);
    if bytes.get(i) == Some(&b']') {
        return None;
    }
    for n in 0.. {
        let end = value_end(bytes, i)?;
        if n == index {
            return Some(Located {
                key: None,
                value: i..end,
            });
        }
        i = skip_ws(bytes, end);
        match bytes.get(i)? {
            b',' => i = skip_ws(bytes, i + 1),
            _ => return None,
        }
    }
    None
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

/// One past the closing quote of the string starting at `start`.
fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut i = start + 1;
    loop {
        match bytes.get(i)? {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
}

/// One past the end of the value starting at `start`.
fn value_end(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'"' => string_end(bytes, start),
        open @ (b'{' | b'[') => {
            let close = if *open == b'{' { b'}' } else { b']' };
            let mut i = skip_ws(bytes, start + 1);
            if bytes.get(i) == Some(&close) {
                return Some(i + 1);
            }
            loop {
                if *open == b'{' {
                    let key_end = string_end(bytes, i)?;
                    let colon = skip_ws(bytes, key_end);
                    if bytes.get(colon) != Some(&b':') {
                        return None;
                    }
                    i = skip_ws(bytes, colon + 1);
                }
                i = skip_ws(bytes, value_end(bytes, i)?);
                match bytes.get(i)? {
                    b',' => i = skip_ws(bytes, i + 1),
                    c if *c == close => return Some(i + 1),
                    _ => return None,
                }
            }
        }
        _ => {
            // A number, `true`, `false` or `null`: runs to the next delimiter.
            let mut i = start;
            while bytes
                .get(i)
                .is_some_and(|c| !matches!(c, b',' | b'}' | b']') && !c.is_ascii_whitespace())
            {
                i += 1;
            }
            (i > start).then_some(i)
        }
    }
}

#[cfg(test)]
mod scanner {
    use super::locate;
    use serde_json::Value;

    /// Every pointer in `value`, depth first.
    fn pointers(value: &Value, at: String, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    pointers(child, format!("{at}/{escaped}"), out);
                }
            }
            Value::Array(items) => {
                for (i, child) in items.iter().enumerate() {
                    pointers(child, format!("{at}/{i}"), out);
                }
            }
            _ => {}
        }
        out.push(at);
    }

    #[test]
    fn every_value_in_every_shipped_scene_is_located_exactly() {
        // The scanner has to agree with a real JSON parser on real files, not
        // only on the cases a test author thought of.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let mut checked = 0;
        for dir in ["examples", "crates/lumina-renderer/tests/fixtures"] {
            for entry in std::fs::read_dir(root.join(dir)).expect("dir").flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("lsf") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("read");
                let doc: Value = serde_json::from_str(&text).expect("json");
                let mut all = Vec::new();
                pointers(&doc, String::new(), &mut all);
                // Every fourth value: each lookup walks from the root, so all
                // 8 000-odd take seconds in a debug build, and a quarter still
                // reaches every kind of value in every file.
                for pointer in all.iter().step_by(4) {
                    let at = locate(&text, pointer)
                        .unwrap_or_else(|| panic!("{}: {pointer} not found", path.display()));
                    let found: Value = serde_json::from_str(&text[at.value.clone()])
                        .unwrap_or_else(|e| panic!("{}: {pointer}: {e}", path.display()));
                    assert_eq!(
                        Some(&found),
                        doc.pointer(pointer),
                        "{}: {pointer}",
                        path.display()
                    );
                }
                checked += all.len().div_ceil(4);
            }
        }
        assert!(checked > 1_500, "only {checked} values checked");
    }
}

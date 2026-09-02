//! "Did you mean …?" for any unknown identifier.
//!
//! The validator already did this for easing names, with Levenshtein distance,
//! and did it for object ids with a three-character prefix match — which
//! misses every transposition (`crcle` never suggests `circle`) and confidently
//! proposes anything that happens to share a prefix (`title` suggests
//! `titan_orbit`). One implementation now serves both, plus the references
//! that had no suggestion at all.
//!
//! This matters more here than in a compiler. A scene is frequently written by
//! a model, and the `fix_suggestion` field is what it acts on: an error saying
//! "check the objects block" costs a round trip that "did you mean `circle`?"
//! does not.

/// The closest candidate to `name`, if one is close enough to be worth saying.
///
/// Returns `None` rather than the least-bad option when nothing is near. A
/// confident wrong suggestion is worse than none: it sends the reader — or the
/// agent — to fix the wrong thing.
#[must_use]
pub fn nearest<'a, I>(name: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    candidates
        .into_iter()
        .map(|c| (edit_distance(name, c), c))
        .min_by_key(|(d, c)| (*d, *c))
        .filter(|(d, c)| *d <= threshold(name, c))
        .map(|(_, c)| c)
}

/// How far apart two names may be and still be worth suggesting.
///
/// Scaled to length rather than fixed. A fixed distance of three calls `abc`
/// and `xyz` a match — every character differs — while rejecting
/// `emitter_positon` for `emitter_position`, which is one transposition in a
/// long word. Roughly a third of the shorter name, capped, and never more than
/// a short name's own length.
fn threshold(a: &str, b: &str) -> usize {
    let shortest = a.chars().count().min(b.chars().count());
    match shortest {
        0..=3 => 1,
        4..=7 => 2,
        _ => (shortest / 3).min(4),
    }
}

/// Classic dynamic-programming Levenshtein distance, over `char`s rather than
/// bytes so a multi-byte character counts as one edit.
#[must_use]
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// A `fix_suggestion` naming the nearest candidate, or `fallback` when nothing
/// is close.
#[must_use]
pub fn did_you_mean<'a, I>(name: &str, candidates: I, fallback: &str) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    nearest(name, candidates)
        .map_or_else(|| fallback.to_string(), |s| format!("Did you mean '{s}'?"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transposition_is_found() {
        // The case the old prefix match could never catch: the first three
        // characters differ, so it returned nothing at all.
        assert_eq!(nearest("crcle", ["circle", "square"]), Some("circle"));
        assert_eq!(
            nearest("emitter_positon", ["emitter_position", "emitter_x"]),
            Some("emitter_position")
        );
    }

    #[test]
    fn a_shared_prefix_is_not_enough_on_its_own() {
        // What the old prefix match got wrong in the other direction: `tit`
        // matches, so it confidently proposed a completely different object.
        assert_eq!(nearest("title", ["titan_orbit_diagram"]), None);
    }

    #[test]
    fn nothing_close_suggests_nothing() {
        // A confident wrong suggestion is worse than none: it sends the reader
        // to fix the wrong thing.
        assert_eq!(nearest("xyz", ["circle", "rectangle"]), None);
        assert_eq!(nearest("background", ["fill", "stroke"]), None);
    }

    #[test]
    fn short_names_are_held_to_a_tighter_threshold() {
        // With a fixed distance of three, `abc` and `xyz` are a match despite
        // sharing nothing.
        assert_eq!(nearest("abc", ["xyz"]), None);
        assert_eq!(nearest("abc", ["abd"]), Some("abd"));
    }

    #[test]
    fn an_exact_match_is_itself() {
        assert_eq!(nearest("circle", ["circle", "circles"]), Some("circle"));
    }

    #[test]
    fn ties_break_deterministically() {
        // Two candidates at the same distance must not depend on iteration
        // order, or the same scene produces different advice between runs —
        // the TD-25 defect in a much smaller place.
        let a = nearest("bat", ["cat", "bad"]);
        let b = nearest("bat", ["bad", "cat"]);
        assert_eq!(a, b, "the suggestion depended on candidate order");
    }

    #[test]
    fn distance_counts_characters_not_bytes() {
        // One multi-byte character replaced by another is one edit, not four.
        assert_eq!(edit_distance("café", "cafe"), 1);
        assert_eq!(edit_distance("αβγ", "αβδ"), 1);
    }

    #[test]
    fn the_fallback_is_used_when_nothing_is_close() {
        assert_eq!(
            did_you_mean("zzz", ["circle"], "Check the objects block."),
            "Check the objects block."
        );
        assert_eq!(
            did_you_mean("crcle", ["circle"], "Check the objects block."),
            "Did you mean 'circle'?"
        );
    }
}

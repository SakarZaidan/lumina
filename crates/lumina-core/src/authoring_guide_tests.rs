//! The authoring guide is the one document a model is told to trust, so it has
//! to keep up with the engine. These hold it to what the engine actually is.

use crate::{authoring_guide, easing::EASING_NAMES, object_registry};

#[test]
fn every_object_type_and_its_required_properties_are_described() {
    let guide = authoring_guide();
    let registry = object_registry();
    let types = registry.as_object().expect("registry");
    assert_eq!(types.len(), 17, "the registry changed size");
    for (name, entry) in types {
        assert!(guide.contains(name), "the guide never mentions {name}");
        let row = guide
            .lines()
            .find(|line| line.starts_with(&format!("| `{name}` |")))
            .unwrap_or_else(|| panic!("the guide has no row for {name}"));
        for required in entry["required"].as_array().expect("required") {
            let property = required.as_str().expect("name");
            assert!(
                row.contains(&format!("`{property}`")),
                "{name}'s row does not require {property}"
            );
        }
    }
}

#[test]
fn every_easing_name_is_listed() {
    let guide = authoring_guide();
    for name in EASING_NAMES {
        assert!(
            guide.contains(&format!("`{name}`")),
            "the guide never lists the easing {name}"
        );
    }
}

#[test]
fn every_error_code_it_names_is_one_the_validator_can_emit() {
    // Read from the validator's own source: a code the guide tells a model to
    // expect, that nothing can produce any more, is worse than an undocumented
    // one.
    let source = include_str!("validation.rs");
    for code in authoring_guide().split('`').filter(|token| {
        token.len() > 3 && token.chars().all(|c| c.is_ascii_uppercase() || c == '_')
    }) {
        assert!(
            source.contains(&format!("\"{code}\"")),
            "the guide names {code}, which the validator does not emit"
        );
    }
}

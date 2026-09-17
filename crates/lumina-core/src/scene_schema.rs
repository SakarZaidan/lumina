//! The scene format's JSON Schema, whole or cut down for a narrow task
//! (`AAA-AI-05`).
//!
//! The full schema is about 38 kB, and most authoring touches two or three of
//! the seventeen object types. A schema *scoped* to those types keeps the whole
//! scene structure — canvas, timeline, events, camera — but only their
//! variants of `Object`, and only the definitions still reachable from what is
//! left. A *compact* schema drops the descriptions, which are about half of its
//! bytes. Both cut what an agent pays to put the schema in its context.
//!
//! Scoping by what the root still reaches, rather than by the requested names,
//! is what keeps the result whole: a schema with a dangling `$ref` is worse
//! than a large one, because a model will either invent the missing type or
//! refuse.

use serde_json::{Map, Value};

/// A requested object type that does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownObjectType {
    /// The name as requested.
    pub name: String,
    /// The nearest real type name, when one is close.
    pub suggestion: Option<String>,
}

impl std::fmt::Display for UnknownObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\" is not an object type", self.name)?;
        if let Some(s) = &self.suggestion {
            write!(f, "; did you mean \"{s}\"?")?;
        }
        Ok(())
    }
}

impl std::error::Error for UnknownObjectType {}

/// The scene format's JSON Schema.
///
/// `objects` restricts it to those object types, by their LSF names such as
/// `"Circle"`; `None` keeps all of them. `compact` drops every description.
///
/// # Errors
///
/// Returns the first requested type that does not exist.
pub fn scene_schema(objects: Option<&[&str]>, compact: bool) -> Result<Value, UnknownObjectType> {
    let mut schema =
        serde_json::to_value(schemars::schema_for!(luminafx_schema::Scene)).unwrap_or(Value::Null);
    if let Some(wanted) = objects {
        restrict_objects(&mut schema, wanted)?;
        prune_unreachable(&mut schema);
    }
    if compact {
        strip_descriptions(&mut schema);
    }
    Ok(schema)
}

/// The definitions map, wherever this schemars version puts it.
fn definitions_mut(schema: &mut Value) -> Option<&mut Map<String, Value>> {
    let key = if schema.get("$defs").is_some() {
        "$defs"
    } else {
        "definitions"
    };
    schema.get_mut(key).and_then(Value::as_object_mut)
}

/// Keep only the `Object` variants named in `wanted`.
fn restrict_objects(schema: &mut Value, wanted: &[&str]) -> Result<(), UnknownObjectType> {
    let Some(variants) = definitions_mut(schema)
        .and_then(|defs| defs.get_mut("Object"))
        .and_then(|object| object.get_mut("oneOf"))
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    let name_of = |variant: &Value| {
        variant
            .pointer("/properties/type/enum/0")
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let names: Vec<String> = variants.iter().filter_map(name_of).collect();
    for name in wanted {
        if !names.iter().any(|n| n == name) {
            return Err(UnknownObjectType {
                name: (*name).to_string(),
                suggestion: crate::suggest::nearest(name, names.iter().map(String::as_str))
                    .map(str::to_string),
            });
        }
    }
    variants.retain(|variant| name_of(variant).is_some_and(|n| wanted.contains(&n.as_str())));
    Ok(())
}

/// Drop every definition the schema no longer reaches from its root.
fn prune_unreachable(schema: &mut Value) {
    let Some(defs) = definitions_mut(schema).map(|d| d.clone()) else {
        return;
    };
    let mut root = schema.clone();
    if let Some(map) = root.as_object_mut() {
        map.remove("definitions");
        map.remove("$defs");
    }
    let mut reached: Vec<String> = Vec::new();
    let mut pending = refs_in(&root);
    while let Some(name) = pending.pop() {
        if reached.contains(&name) {
            continue;
        }
        if let Some(def) = defs.get(&name) {
            pending.extend(refs_in(def));
        }
        reached.push(name);
    }
    if let Some(defs) = definitions_mut(schema) {
        defs.retain(|name, _| reached.contains(name));
    }
}

/// Every definition name a `$ref` in `value` points at, at any depth.
fn refs_in(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map
            .iter()
            .flat_map(|(key, v)| match (key.as_str(), v.as_str()) {
                ("$ref", Some(r)) => r
                    .rsplit('/')
                    .next()
                    .map(str::to_string)
                    .into_iter()
                    .collect(),
                _ => refs_in(v),
            })
            .collect(),
        Value::Array(items) => items.iter().flat_map(refs_in).collect(),
        _ => Vec::new(),
    }
}

/// Remove every `description` keyword, without touching a property that
/// happens to be called `description`.
fn strip_descriptions(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            map.remove("description");
            for (key, value) in map.iter_mut() {
                // Under these keys the names are data, not keywords: recurse
                // into each named schema, but never strip the name itself.
                if matches!(
                    key.as_str(),
                    "properties" | "definitions" | "$defs" | "patternProperties"
                ) {
                    if let Some(named) = value.as_object_mut() {
                        for sub in named.values_mut() {
                            strip_descriptions(sub);
                        }
                    }
                } else {
                    strip_descriptions(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_descriptions),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{refs_in, scene_schema, strip_descriptions, UnknownObjectType};
    use serde_json::{json, Value};

    fn definitions(schema: &Value) -> &serde_json::Map<String, Value> {
        schema
            .get("definitions")
            .or_else(|| schema.get("$defs"))
            .and_then(Value::as_object)
            .expect("definitions")
    }

    fn assert_whole(schema: &Value) {
        let defs = definitions(schema);
        for name in refs_in(schema) {
            assert!(
                defs.contains_key(&name),
                "`{name}` is referenced but not defined"
            );
        }
    }

    #[test]
    fn with_no_options_it_is_the_schema_the_types_generate() {
        let generated =
            serde_json::to_value(schemars::schema_for!(luminafx_schema::Scene)).expect("schema");
        assert_eq!(scene_schema(None, false).expect("schema"), generated);
    }

    #[test]
    fn a_scoped_schema_keeps_the_scene_and_only_the_requested_objects() {
        // Requested by the LSF names an author writes, not the Rust struct
        // names — the mistake the MCP tool shipped with, where `Circle` pruned
        // every definition and left the root pointing at nothing.
        let scoped = scene_schema(Some(&["Circle", "Text"]), false).expect("schema");
        assert_whole(&scoped);
        let defs = definitions(&scoped);
        for kept in [
            "CircleProps",
            "TextProps",
            "Paint",
            "Shadow",
            "Canvas",
            "TimelineEntry",
        ] {
            assert!(defs.contains_key(kept), "lost {kept}");
        }
        for dropped in ["RectangleProps", "AxesProps", "ParticlesProps"] {
            assert!(!defs.contains_key(dropped), "kept {dropped}");
        }
        let variants = defs["Object"]["oneOf"].as_array().expect("variants");
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn scoping_and_compacting_make_it_a_fraction_of_the_size() {
        let full = scene_schema(None, false).expect("schema").to_string().len();
        let small = scene_schema(Some(&["Circle"]), true).expect("schema");
        assert_whole(&small);
        let small = small.to_string().len();
        assert!(small * 3 < full, "{small} bytes of {full}");
    }

    #[test]
    fn an_unknown_type_is_named_with_a_suggestion() {
        assert_eq!(
            scene_schema(Some(&["Cirle"]), false),
            Err(UnknownObjectType {
                name: "Cirle".to_string(),
                suggestion: Some("Circle".to_string()),
            })
        );
    }

    #[test]
    fn compacting_removes_descriptions_but_not_properties_named_description() {
        let mut schema = json!({
            "description": "gone",
            "properties": {
                "description": { "type": "string", "description": "gone too" }
            }
        });
        strip_descriptions(&mut schema);
        assert_eq!(
            schema,
            json!({ "properties": { "description": { "type": "string" } } })
        );
        let compact = scene_schema(None, true).expect("schema").to_string();
        assert!(!compact.contains("\"description\""));
    }
}

//! What properties each object type accepts, and what JSON types they take —
//! read from the real structs rather than written down a second time.
//!
//! RFC-0002 Stage 1. The permitted properties for `Circle` are whatever
//! `CircleProps` declares, and nothing else is allowed to say otherwise: a
//! hand-maintained table would drift the first time someone added a field to
//! one and not the other. So this derives the table from `schemars`' output.
//!
//! # Why it walks JSON, not schemars' Rust types
//!
//! It reads the *emitted JSON Schema* rather than schemars' `Schema` structs.
//! Those structs changed heavily between schemars 0.8 and 1.x (issue #50), but
//! the JSON Schema they emit is a published standard. Reading the standard is
//! what keeps a schemars upgrade confined to this one file.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// A set of JSON value kinds a property accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JsonKinds(u8);

impl JsonKinds {
    const NUMBER: u8 = 1 << 0;
    const STRING: u8 = 1 << 1;
    const BOOL: u8 = 1 << 2;
    const ARRAY: u8 = 1 << 3;
    const OBJECT: u8 = 1 << 4;
    const NULL: u8 = 1 << 5;
    /// `integer` in the schema. A property whose only numbers are integers
    /// still *accepts* any number — see `type_bit` — but is rounded when
    /// animated, because a fractional value will not deserialise into it.
    const INTEGER: u8 = 1 << 6;
    /// Resolution failed somewhere, so nothing may be rejected on type grounds.
    const ANY: u8 = 1 << 7;

    /// Whether a value of this kind is acceptable.
    #[must_use]
    pub fn accepts(self, value: &Value) -> bool {
        if self.0 & Self::ANY != 0 {
            return true;
        }
        let bit = match value {
            Value::Number(_) => Self::NUMBER | Self::INTEGER,
            Value::String(_) => Self::STRING,
            Value::Bool(_) => Self::BOOL,
            Value::Array(_) => Self::ARRAY,
            Value::Object(_) => Self::OBJECT,
            Value::Null => Self::NULL,
        };
        self.0 & bit != 0
    }

    /// Whether every number this property takes is an integer, as for
    /// `z_index` or a particle `count`.
    ///
    /// False when resolution failed, and false when the schema also allows a
    /// fractional number: rounding is only safe when nothing but an integer
    /// can be meant.
    #[must_use]
    pub fn is_integer(self) -> bool {
        self.0 & (Self::INTEGER | Self::NUMBER | Self::ANY) == Self::INTEGER
    }

    /// Human-readable description of what is accepted, for error messages.
    #[must_use]
    pub fn describe(self) -> String {
        if self.0 & Self::ANY != 0 {
            return "any value".to_string();
        }
        let mut names = Vec::new();
        for (bit, name) in [
            (Self::NUMBER | Self::INTEGER, "a number"),
            (Self::STRING, "a string"),
            (Self::BOOL, "a boolean"),
            (Self::ARRAY, "an array"),
            (Self::OBJECT, "an object"),
        ] {
            if self.0 & bit != 0 {
                names.push(name);
            }
        }
        // `null` is an implementation detail of `Option`; naming it in an error
        // would suggest "null" as a fix for a wrong-typed value, which it never
        // is.
        match names.len() {
            0 => "null".to_string(),
            1 => names[0].to_string(),
            _ => {
                let last = names.pop().unwrap_or_default();
                format!("{} or {last}", names.join(", "))
            }
        }
    }
}

/// Describe a JSON value's kind, for error messages.
#[must_use]
pub fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Bool(_) => "a boolean",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
        Value::Null => "null",
    }
}

/// Every object type's properties, derived once from the schema.
#[derive(Debug, Default)]
pub struct PropertySchema {
    types: HashMap<String, HashMap<String, JsonKinds>>,
}

impl PropertySchema {
    /// The schema for this build of the engine, built on first use.
    ///
    /// Generated once per process and then shared: validation runs per scene,
    /// and regenerating a 33-definition JSON Schema per call would cost more
    /// than validating the scene.
    pub fn get() -> &'static PropertySchema {
        static SCHEMA: OnceLock<PropertySchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let root = serde_json::to_value(schemars::schema_for!(luminafx_schema::Scene))
                .unwrap_or(Value::Null);
            PropertySchema::from_json_schema(&root)
        })
    }

    /// Build from an emitted JSON Schema document.
    ///
    /// Public for tests; everything else should use [`PropertySchema::get`].
    #[must_use]
    pub fn from_json_schema(root: &Value) -> PropertySchema {
        let defs = root
            .get("definitions")
            .or_else(|| root.get("$defs"))
            .and_then(Value::as_object);
        let Some(defs) = defs else {
            return PropertySchema::default();
        };

        let mut types = HashMap::new();
        // `Object` is a tagged enum: a `oneOf` whose members pin `type` to a
        // single string and point `properties` at the props definition.
        let variants = defs
            .get("Object")
            .and_then(|o| o.get("oneOf"))
            .and_then(Value::as_array);
        for variant in variants.into_iter().flatten() {
            let Some(name) = variant
                .pointer("/properties/type/enum/0")
                .and_then(Value::as_str)
            else {
                continue;
            };
            let Some(props_ref) = variant
                .pointer("/properties/properties/$ref")
                .and_then(Value::as_str)
            else {
                continue;
            };
            let Some(props_def) = resolve_ref(defs, props_ref) else {
                continue;
            };
            let mut props = HashMap::new();
            if let Some(fields) = props_def.get("properties").and_then(Value::as_object) {
                for (field, schema) in fields {
                    props.insert(field.clone(), kinds_of(defs, schema, 0));
                }
            }
            types.insert(name.to_string(), props);
        }
        PropertySchema { types }
    }

    /// Every object type name, e.g. `"Circle"`.
    pub fn object_types(&self) -> impl Iterator<Item = &str> {
        self.types.keys().map(String::as_str)
    }

    /// The properties of `object_type`, or `None` if there is no such type.
    #[must_use]
    pub fn properties_of(&self, object_type: &str) -> Option<&HashMap<String, JsonKinds>> {
        self.types.get(object_type)
    }
}

/// Look up a `#/definitions/Name` reference.
fn resolve_ref<'a>(defs: &'a serde_json::Map<String, Value>, reference: &str) -> Option<&'a Value> {
    defs.get(reference.rsplit('/').next()?)
}

/// The JSON kinds a sub-schema accepts.
///
/// Anything this cannot resolve with confidence returns `ANY`, so a gap in the
/// resolver makes validation more permissive rather than rejecting a scene the
/// engine would render correctly. A false "wrong type" error on a valid scene
/// is worse than a missed one, because the author then has no correct answer.
fn kinds_of(defs: &serde_json::Map<String, Value>, schema: &Value, depth: usize) -> JsonKinds {
    // Recursive definitions are legal JSON Schema; a depth bound stops a cycle
    // from overflowing the stack rather than trusting the schema not to have one.
    if depth > 16 {
        return JsonKinds(JsonKinds::ANY);
    }
    let mut bits = 0u8;

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return resolve_ref(defs, reference)
            .map_or(JsonKinds(JsonKinds::ANY), |d| kinds_of(defs, d, depth + 1));
    }

    match schema.get("type") {
        Some(Value::String(t)) => bits |= type_bit(t),
        Some(Value::Array(ts)) => {
            for t in ts.iter().filter_map(Value::as_str) {
                bits |= type_bit(t);
            }
        }
        _ => {}
    }

    // `enum` without a `type` is a set of literals; take their kinds.
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        for v in values {
            bits |= match v {
                Value::String(_) => JsonKinds::STRING,
                Value::Number(_) => JsonKinds::NUMBER,
                Value::Bool(_) => JsonKinds::BOOL,
                Value::Null => JsonKinds::NULL,
                Value::Array(_) => JsonKinds::ARRAY,
                Value::Object(_) => JsonKinds::OBJECT,
            };
        }
    }

    // Composition keywords. `allOf` is treated as a union too: in practice
    // schemars emits it as a single-member wrapper around a `$ref`, and a true
    // intersection would need a full JSON Schema evaluator to get right.
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(members) = schema.get(key).and_then(Value::as_array) {
            for member in members {
                bits |= kinds_of(defs, member, depth + 1).0;
            }
        }
    }

    // A schema with `properties` and no explicit type still describes an
    // object.
    if bits == 0 && schema.get("properties").is_some() {
        bits |= JsonKinds::OBJECT;
    }

    if bits == 0 {
        JsonKinds(JsonKinds::ANY)
    } else {
        JsonKinds(bits)
    }
}

fn type_bit(t: &str) -> u8 {
    match t {
        "number" => JsonKinds::NUMBER,
        // Recorded separately so the timeline can round an animated integer,
        // but `accepts` lets any number through: rejecting `2.5` for `z_index`
        // would reject a scene the engine renders correctly, rounded.
        "integer" => JsonKinds::INTEGER,
        "string" => JsonKinds::STRING,
        "boolean" => JsonKinds::BOOL,
        "array" => JsonKinds::ARRAY,
        "object" => JsonKinds::OBJECT,
        "null" => JsonKinds::NULL,
        _ => JsonKinds::ANY,
    }
}

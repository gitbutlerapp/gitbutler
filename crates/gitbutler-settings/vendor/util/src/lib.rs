use std::borrow::Cow;

use rust_embed::Embed;

pub fn merge_non_null_json_value_into(source: serde_json::Value, target: &mut serde_json::Value) {
    use serde_json::Value;
    if let Value::Object(source_object) = source {
        let target_object = if let Value::Object(target) = target {
            target
        } else {
            *target = Value::Object(Default::default());
            target.as_object_mut().unwrap()
        };
        for (key, value) in source_object {
            if let Some(target) = target_object.get_mut(&key) {
                merge_non_null_json_value_into(value, target);
            } else if !value.is_null() {
                target_object.insert(key.clone(), value);
            }
        }
    } else if !source.is_null() {
        *target = source
    }
}

/// Get an embedded file as a string.
pub fn asset_str<A: Embed>(path: &str) -> Cow<'static, str> {
    match A::get(path).unwrap().data {
        Cow::Borrowed(bytes) => Cow::Borrowed(std::str::from_utf8(bytes).unwrap()),
        Cow::Owned(bytes) => Cow::Owned(String::from_utf8(bytes).unwrap()),
    }
}

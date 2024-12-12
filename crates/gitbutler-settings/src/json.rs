// Given a current json value and an update json value, return a json value that represents the difference between the two.
pub fn json_difference(
    current: serde_json::Value,
    update: &serde_json::Value,
) -> serde_json::Value {
    use serde_json::Value;
    if let Value::Object(update_object) = &update {
        if let Value::Object(current_object) = current {
            let mut result = serde_json::Map::new();
            for (key, update_value) in update_object {
                if let Some(current_value) = current_object.get(key) {
                    if current_value != update_value {
                        result.insert(
                            key.clone(),
                            json_difference(current_value.clone(), update_value),
                        );
                    }
                } else {
                    result.insert(key.clone(), update_value.clone());
                }
            }
            Value::Object(result)
        } else {
            update.clone()
        }
    } else {
        update.clone()
    }
}

/// Based on Zed `merge_non_null_json_value_into`
/// Note: This doesn't merge arrays.
pub fn merge_non_null_json_value(source: serde_json::Value, target: &mut serde_json::Value) {
    use serde_json::Value;
    if let Value::Object(source_object) = source {
        let target_object = if let Value::Object(target) = target {
            target
        } else {
            *target = serde_json::json!({});
            target.as_object_mut().expect("object was just set")
        };
        for (key, value) in source_object {
            if let Some(target) = target_object.get_mut(&key) {
                merge_non_null_json_value(value, target);
            } else if !value.is_null() {
                target_object.insert(key, value);
            }
        }
    } else if !source.is_null() {
        *target = source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_does_not_merge_null_values() {
        let source = serde_json::json!({"a": null, "b": true });
        let mut target = serde_json::json!({});
        merge_non_null_json_value(source, &mut target);
        assert_eq!(target, serde_json::json!({"b": true }));
    }

    #[test]
    fn it_does_not_merge_arrays() {
        let source = serde_json::json!({"a": null, "b": [1,2,3]});
        let mut target = serde_json::json!({"a": {"b": 1}, "b": [42]});
        merge_non_null_json_value(source, &mut target);
        assert_eq!(target, serde_json::json!({"a": {"b": 1}, "b": [1,2,3] }));
    }

    #[test]
    fn it_merges_nested_objects_correctly() {
        let source = serde_json::json!({"a": {"b": {"c": 42}}});
        let mut target = serde_json::json!({});
        merge_non_null_json_value(source.clone(), &mut target);
        assert_eq!(target, source);
    }

    #[test]
    pub fn test_difference_existing_key() {
        use serde_json::json;
        let current = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 4
            }
        });
        let update = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 5
            }
        });
        assert_eq!(
            json_difference(current, &update),
            json!({
                "e": {
                    "f": 5
                }
            })
        );
    }

    #[test]
    pub fn test_difference_new_key() {
        use serde_json::json;
        let current = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 4
            }
        });
        let update = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 4
            },
            "g": 5
        });
        assert_eq!(
            json_difference(current, &update),
            json!({
                "g": 5
            })
        );
    }

    #[test]
    pub fn test_no_overlap_at_all() {
        use serde_json::json;
        let current = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 4
            }
        });
        let update = json!({
            "g": 5,
            "h": {
                "i": 6,
                "j": 7
            },
            "k": {
                "l": 8
            }
        });
        assert_eq!(json_difference(current, &update), update);
    }

    #[test]
    pub fn test_everything_is_same_noop() {
        use serde_json::json;
        let current = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 4
            }
        });
        let update = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 4
            }
        });
        assert_eq!(json_difference(current, &update), json!({}));
    }

    #[test]
    pub fn test_difference_new_key_with_null() {
        use serde_json::json;
        let current = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": 4
            }
        });
        let update = json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": {
                "f": null
            },
            "g": 5
        });
        assert_eq!(
            json_difference(current, &update),
            json!({
                "e": {
                    "f": null
                },
                "g": 5
            })
        );
    }

    #[test]
    pub fn test_both_null() {
        use serde_json::json;
        let current = json!({
            "a": null
        });
        let update = json!({
            "a": null
        });
        assert_eq!(json_difference(current, &update), json!({}));
    }

    #[test]
    pub fn test_empty_object() {
        use serde_json::json;
        let current = json!({});
        let update = json!({});
        assert_eq!(json_difference(current, &update), json!({}));
    }

    #[test]
    pub fn test_empty_object_with_new_key() {
        use serde_json::json;
        let current = json!({});
        let update = json!({
            "a": 1
        });
        assert_eq!(json_difference(current, &update), update);
    }
}

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

// tests
mod tests {
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
            super::json_difference(current, &update),
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
            super::json_difference(current, &update),
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
        assert_eq!(super::json_difference(current, &update), update);
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
        assert_eq!(super::json_difference(current, &update), json!({}));
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
            super::json_difference(current, &update),
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
        assert_eq!(super::json_difference(current, &update), json!({}));
    }

    #[test]
    pub fn test_empty_object() {
        use serde_json::json;
        let current = json!({});
        let update = json!({});
        assert_eq!(super::json_difference(current, &update), json!({}));
    }

    #[test]
    pub fn test_empty_object_with_new_key() {
        use serde_json::json;
        let current = json!({});
        let update = json!({
            "a": 1
        });
        assert_eq!(super::json_difference(current, &update), update);
    }
}

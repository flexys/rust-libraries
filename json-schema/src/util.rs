use serde_json::Value;

/// Determines if a specific field within the "properties" section of a JSON schema is a date field.
///
/// It checks if the specified field is a string and has the format "date".
///
/// # Arguments
///
/// * `schema`: A reference to the JSON schema `Value`.
/// * `field_name`: The name of the field to check within the "properties" section.
///
/// # Returns
///
/// `true` if the specified field is a date field, `false` otherwise.
/// Returns `false` if the field or "properties" section does not exist.
///
pub fn is_date_field(schema: &Value, field_name: &str) -> bool {
    if !schema.is_object() {
        return false;
    }

    if let Some(properties) = schema.get("properties") {
        if !properties.is_object() {
            return false;
        }
        if let Some(field_value) = properties.get(field_name) {
            if let Some(type_value) = field_value.get("type") {
                if let Some(type_str) = type_value.as_str() {
                    if type_str != "string" {
                        return false;
                    }
                } else {
                    return false;
                }
            } else {
                return false;
            }

            if let Some(format_value) = field_value.get("format") {
                if let Some(format_str) = format_value.as_str() {
                    if format_str == "date" {
                        return true;
                    }
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_date_field_valid() {
        let schema = json!({
            "properties": {
                "date_field": {
                    "type": "string",
                    "format": "date"
                }
            }
        });
        assert_eq!(is_date_field(&schema, "date_field"), true);
    }

    #[test]
    fn test_is_date_field_wrong_type() {
        let schema = json!({
            "properties": {
                "date_field": {
                    "type": "integer",
                    "format": "date"
                }
            }
        });
        assert_eq!(is_date_field(&schema, "date_field"), false);
    }

    #[test]
    fn test_is_date_field_wrong_format() {
        let schema = json!({
            "properties": {
                "date_field": {
                    "type": "string",
                    "format": "email"
                }
            }
        });
        assert_eq!(is_date_field(&schema, "date_field"), false);
    }

    #[test]
    fn test_is_date_field_missing_type() {
        let schema = json!({
            "properties": {
                "date_field": {
                    "format": "date"
                }
            }
        });
        assert_eq!(is_date_field(&schema, "date_field"), false);
    }

    #[test]
    fn test_is_date_field_missing_format() {
        let schema = json!({
            "properties": {
                "date_field": {
                    "type": "string"
                }
            }
        });
        assert_eq!(is_date_field(&schema, "date_field"), false);
    }

    #[test]
    fn test_is_date_field_field_not_found() {
        let schema = json!({
            "properties": {}
        });
        assert_eq!(is_date_field(&schema, "non_existent_field"), false);
    }

    #[test]
    fn test_is_date_field_schema_not_object() {
        let schema = json!([]);
        assert_eq!(is_date_field(&schema, "any_field"), false);
    }

    #[test]
    fn test_is_date_field_no_properties() {
        let schema = json!({});
        assert_eq!(is_date_field(&schema, "any_field"), false);
    }

    #[test]
    fn test_is_date_field_properties_not_object() {
        let schema = json!({
            "properties": []
        });
        assert_eq!(is_date_field(&schema, "any_field"), false);
    }
}

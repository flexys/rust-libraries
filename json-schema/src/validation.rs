use anyhow::{anyhow, bail, Result};
use jsonschema::validator_for;
use serde_json::Value;

pub fn validate_json(schema: &Value, inputs: &Value) -> Result<()> {
    let validator =
        validator_for(schema).map_err(|err| anyhow!("Invalid json schema, error: {err}"))?;

    let validation = validator.validate(inputs);

    if validation.is_err() {
        let error_msg = validator
            .iter_errors(inputs)
            .map(|error_instance| {
                format!(
                    "Validation Error [{}]. Schema Path [{}]. Instance Path [{}]. Instance: {}",
                    error_instance,
                    error_instance.schema_path,
                    error_instance.instance_path,
                    serde_json::to_string_pretty(&error_instance.instance).unwrap()
                )
            })
            .reduce(|total_errors, error_line| total_errors + ", " + error_line.as_str())
            .unwrap_or("Missing Error".to_string());

        bail!("Json failed validation, error(s): {error_msg}")
    };

    Ok(())
}

/// Merge two json object into a single object with combined keys.
/// Where two objects share a key, the second value wins.
/// If the values passed aren't objects, returns an error.
pub fn merge_json_objects(a: Value, b: Value) -> Result<Value> {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            let merged_map = a.into_iter().chain(b).collect();

            Ok(Value::Object(merged_map))
        }
        (Value::Object(_), b) => {
            let b_prettified = serde_json::to_string_pretty(&b)?;
            bail!("value required to be object to merge. Instead got {b_prettified}");
        }
        (a, Value::Object(_)) => {
            let a_prettified = serde_json::to_string_pretty(&a)?;
            bail!("value required to be object to merge. Instead got {a_prettified}");
        }
        (a, b) => {
            let a_prettified = serde_json::to_string_pretty(&a)?;
            let b_prettified = serde_json::to_string_pretty(&b)?;

            bail!("Both values required to be object to merge. Neither value was an object. Instead got: {a_prettified} AND {b_prettified}")
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_json_diff::{assert_json_matches, CompareMode, Config};
    use assertables::assert_starts_with;
    use serde_json::json;

    use super::{merge_json_objects, validate_json};

    #[test]
    fn test_validate_json_errors_messages_contain_paths() {
        let schema = json!({
          "type": "object",
          "$schema": "http://json-schema.org/draft-07/schema#",
          "properties": {
            "x": {
              "type": "number"
            },
            "y": {
              "type": "number"
            }
          }
        });
        let inputs = json!({
          "x": "wibble",
          "y": "wobble"
        });

        let result = validate_json(&schema, &inputs);
        assert!(result.is_err());
        assert_starts_with!(
            result.unwrap_err().to_string(),
            r#"Json failed validation, error(s): Validation Error ["wibble" is not of type "number"]. Schema Path [/properties/x/type]. Instance Path [/x]. Instance:"#
        );
    }

    #[test]
    fn validate_workflow_input_accepts_valid_inputs() {
        let schema = json!({
            "type": "object",
            "properties":{
                "testKey": {
                    "type": "string"
                }
            },
            "required": ["testKey"]
        });
        let inputs = json!({
            "testKey": "testValue"
        });

        let result = validate_json(&schema, &inputs);
        assert_eq!(result.ok(), Some(()));
    }

    #[test]
    fn validate_workflow_input_rejects_invalid_inputs() {
        let schema = json!({
            "type": "object",
            "properties":{
                "requiredKey": {
                    "type": "string"
                }
            },
            "required": ["requiredKey"]
        });
        let inputs = json!({
            "wrongKey": "testValue"
        });

        let result = validate_json(&schema, &inputs);
        assert!(result.is_err());
        assert_starts_with!(
            result.unwrap_err().to_string(),
            r#"Json failed validation, error(s): Validation Error ["requiredKey" is a required property]. Schema Path [/required]. Instance Path []. Instance:"#
        )
    }

    #[test]
    fn merge_json_objects_returns_failure_when_first_value_is_not_an_object() {
        let obj1 = json!("foo");

        let obj2 = json!({
            "baz": "bing"
        });

        let result = merge_json_objects(obj1, obj2);

        assert_eq!(
            result.unwrap_err().to_string(),
            "value required to be object to merge. Instead got \"foo\""
        )
    }

    #[test]
    fn merge_json_objects_returns_failure_when_second_value_is_not_an_object() {
        let obj1 = json!({
            "foo": "bar"
        });

        let obj2 = json!(1);

        let result = merge_json_objects(obj1, obj2);

        assert_eq!(
            result.unwrap_err().to_string(),
            "value required to be object to merge. Instead got 1"
        )
    }

    #[test]
    fn merge_json_objects_merges_two_disparate_objects() {
        let obj1 = json!({
            "foo": "bar"
        });

        let obj2 = json!({
            "baz": "bing"
        });

        let result =
            merge_json_objects(obj1, obj2).expect("Two objects should have merged successfully");

        assert_json_matches!(
            result,
            json!({ "foo": "bar", "baz": "bing" }),
            Config::new(CompareMode::Strict)
        );
    }

    #[test]
    fn merge_json_objects_merges_two_objects_with_collision_favours_the_second() {
        let obj1 = json!({
            "foo": "bar"
        });

        let obj2 = json!({
            "foo": "boom",
            "baz": "bing"
        });

        let result =
            merge_json_objects(obj1, obj2).expect("Two objects should have merged successfully");

        assert_json_matches!(
            result,
            json!({ "foo": "boom", "baz": "bing" }),
            Config::new(CompareMode::Strict)
        );
    }
}

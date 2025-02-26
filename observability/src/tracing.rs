use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Debug;
use tracing::field::Field;
use tracing_subscriber::{registry::LookupSpan, Layer};

#[derive(Debug, Serialize)]
struct LogOutput<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a Value>,
    name: &'a str,
    severity: &'a str,
    parent: &'a str,
    fields: &'a BTreeMap<String, Value>,
    target: &'a str,
}

#[derive(Debug, Clone)]
struct JsonFieldStorage {
    storage: BTreeMap<String, Value>,
}

pub struct FlatJsonLayer {}

impl FlatJsonLayer {
    fn collect_span_fields(
        span_storages: impl Iterator<Item = Option<BTreeMap<String, Value>>>,
    ) -> BTreeMap<String, Value> {
        let mut span_fields = BTreeMap::new();
        span_storages.for_each(|storage| {
            if let Some(mut storage_data) = storage {
                span_fields.append(&mut storage_data)
            }
        });
        span_fields
    }

    fn build_output(
        payload: &BTreeMap<String, Value>,
        target: &str,
        name: &str,
        severity: &str,
        parent: &str,
    ) -> Value {
        let message = payload.get("message");

        serde_json::json!(LogOutput {
            fields: payload,
            message,
            name,
            severity,
            parent,
            target
        })
    }
}

impl<S> Layer<S> for FlatJsonLayer
where
    S: tracing::Subscriber,
    S: for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut fields = BTreeMap::new();
        let mut visitor = JsonVisitor {
            storage: &mut fields,
        };
        attrs.record(&mut visitor);

        let storage = JsonFieldStorage { storage: fields };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert::<JsonFieldStorage>(storage);
        }
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(json_storage) = extensions.get_mut::<JsonFieldStorage>() {
                let json_data = &mut json_storage.storage;
                let mut visitor = JsonVisitor { storage: json_data };
                values.record(&mut visitor);
            }
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = if let Some(scope) = ctx.event_scope(event) {
            let mapping = scope.from_root().map(|item| {
                item.extensions()
                    .get::<JsonFieldStorage>()
                    .map(|storage_data| storage_data.storage.clone())
            });
            Self::collect_span_fields(mapping)
        } else {
            BTreeMap::new()
        };

        let mut visitor = JsonVisitor {
            storage: &mut fields,
        };

        event.record(&mut visitor);

        let parent = ctx
            .event_span(event)
            .map(|parent_span| parent_span.name())
            .unwrap_or("ROOT");

        let payload = &fields;

        let output = Self::build_output(
            payload,
            event.metadata().target(),
            event.metadata().name(),
            format!("{}", event.metadata().level()).as_str(),
            parent,
        );

        println!("{}", serde_json::to_string(&output).unwrap());
    }
}

struct JsonVisitor<'a> {
    storage: &'a mut BTreeMap<String, Value>,
}

impl JsonVisitor<'_> {
    fn record_debug_as_json(&mut self, field_name: String, value: &dyn Debug) {
        let value_str = format!("{:?}", value);
        let json_value: Value =
            serde_json::from_str(value_str.as_str()).unwrap_or(serde_json::json!(value_str));
        self.storage.insert(field_name, json_value);
    }

    fn insert_json(&mut self, field_name: String, value: impl Serialize) {
        self.storage.insert(field_name, serde_json::json!(value));
    }
}

impl tracing::field::Visit for JsonVisitor<'_> {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.insert_json(field.name().into(), value)
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert_json(field.name().into(), value)
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert_json(field.name().into(), value)
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.insert_json(field.name().into(), value)
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.insert_json(field.name().into(), value)
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert_json(field.name().into(), value)
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert_json(field.name().into(), value)
    }

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        self.insert_json(field.name().into(), value)
    }

    fn record_error(&mut self, field: &Field, value: &(dyn Error + 'static)) {
        self.insert_json(field.name().into(), value.to_string())
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.record_debug_as_json(field.name().into(), value);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use valuable::Valuable;
    use valuable_serde::Serializable;

    use super::*;

    #[test]
    fn context_builder_produces_flattened_map() {
        let span_storages = vec![
            Some(BTreeMap::from([
                (
                    "map1Key".to_string(),
                    json!({"map1NestedKey": "map1NestedValue"}),
                ),
                (
                    "map1Key2".to_string(),
                    json!({"map1NestedKey2": "map1NestedValue2"}),
                ),
            ])),
            Some(BTreeMap::from([(
                "map2Key".to_string(),
                json!({"map2NestedKey1": "map2NestedValue1", "map2NestedKey2": "map2NestedValue2"}),
            )])),
            None,
        ];
        let resulting_context = FlatJsonLayer::collect_span_fields(span_storages.into_iter());

        assert_eq!(
            resulting_context,
            BTreeMap::from([
                (
                    "map1Key".to_string(),
                    json!({"map1NestedKey": "map1NestedValue"})
                ),
                (
                    "map1Key2".to_string(),
                    json!({"map1NestedKey2": "map1NestedValue2"})
                ),
                (
                    "map2Key".to_string(),
                    json!({"map2NestedKey1": "map2NestedValue1", "map2NestedKey2": "map2NestedValue2"})
                )
            ])
        )
    }

    #[test]
    fn trace_output_message_is_message_from_payload() {
        let payload = BTreeMap::from([
            ("message".to_string(), json!("this is a message")),
            ("different_field".to_string(), json!(123)),
        ]);
        let resulting_value =
            FlatJsonLayer::build_output(&payload, "testTarget", "testName", "INFO", "testParent");
        assert_eq!(
            resulting_value.get("message"),
            Some(&Value::String("this is a message".into()))
        )
    }

    #[test]
    fn trace_output_contains_log_fields() {
        let payload = BTreeMap::from([
            ("message".to_string(), json!("this is a message")),
            ("different_field".to_string(), json!(123)),
        ]);
        let resulting_value =
            FlatJsonLayer::build_output(&payload, "testTarget", "testName", "INFO", "testParent");
        assert_eq!(
            resulting_value.get("fields"),
            Some(&json!({
                "message": "this is a message",
                "different_field": 123
            }))
        )
    }

    #[test]
    fn trace_output_does_not_include_none_message() {
        let payload = BTreeMap::from([
            ("fieldA".to_string(), json!(123)),
            ("fieldB".to_string(), json!(123)),
        ]);
        let resulting_value =
            FlatJsonLayer::build_output(&payload, "testTarget", "testName", "INFO", "testParent");
        assert_eq!(resulting_value.get("message"), None)
    }

    struct DisplayValue<T: std::fmt::Display>(T);
    impl<T: std::fmt::Display> Debug for DisplayValue<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Display::fmt(&self.0, f)
        }
    }

    #[test]
    fn visitor_records_json_for_debug_implementation() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };
        let json_value = json!({
            "key1":"value1",
            "key2":"value2",
        });
        let json_value_as_display = DisplayValue(&json_value);

        json_visitor.record_debug_as_json("new_field".into(), &json_value_as_display);

        assert_eq!(storage.get("new_field"), Some(&json_value));
    }

    #[derive(Debug)]
    struct TestError;
    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self)
        }
    }
    impl Error for TestError {}

    #[test]
    fn visitor_records_error_as_string() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("error".into(), TestError.to_string());
        assert_eq!(
            json_visitor.storage.get("error"),
            Some(&json!(TestError.to_string()))
        );
    }

    #[test]
    fn visitor_records_f64() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("f64".into(), 2.5_f64);
        assert_eq!(json_visitor.storage.get("f64"), Some(&json!(2.5_f64)));
    }

    #[test]
    fn visitor_records_i64() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("i64".into(), 2_i64);
        assert_eq!(json_visitor.storage.get("i64"), Some(&json!(2_i64)));
    }

    #[test]
    fn visitor_records_u64() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("u64".into(), 2_u64);
        assert_eq!(json_visitor.storage.get("u64"), Some(&json!(2_u64)));
    }

    #[test]
    fn visitor_records_bool() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("bool".into(), true);
        assert_eq!(json_visitor.storage.get("bool"), Some(&json!(true)));
    }

    #[test]
    fn visitor_records_str() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("str".into(), "str_value");
        assert_eq!(json_visitor.storage.get("str"), Some(&json!("str_value")));
    }

    #[test]
    fn visitor_records_i128() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("i128".into(), 2_i128);
        assert_eq!(json_visitor.storage.get("i128"), Some(&json!(2_i128)));
    }

    #[test]
    fn visitor_records_u128() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        json_visitor.insert_json("u128".into(), 2_u128);
        assert_eq!(json_visitor.storage.get("u128"), Some(&json!(2_u128)));
    }

    #[derive(Valuable)]
    struct TestValuable {
        key1: String,
        key2: i32,
    }

    #[test]
    fn visitor_records_valuable_as_serde_json() {
        let mut storage = BTreeMap::new();
        let mut json_visitor = JsonVisitor {
            storage: &mut storage,
        };

        let valuable_data = TestValuable {
            key1: "test1".into(),
            key2: 1234,
        };

        let serializable_value = Serializable::new(&valuable_data);
        json_visitor.insert_json("valuable".into(), serializable_value);

        assert_eq!(
            json_visitor.storage.get("valuable"),
            Some(&json!({
                "key1": "test1",
                "key2": 1234,
            }))
        );
    }
}

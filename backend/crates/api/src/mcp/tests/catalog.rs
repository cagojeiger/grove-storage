use super::*;
use grove_management_command::{CommandName, Effect};
use std::collections::BTreeSet;

#[test]
fn tools_match_all_command_inputs_and_preserve_schema_references() {
    let mut names = BTreeSet::new();
    for &name in CommandName::ALL {
        let tool = contract::tool(name);
        assert!(names.insert(tool.name.to_string()));
        assert_eq!(
            Value::Object((*tool.input_schema).clone()),
            name.input_schema().to_value()
        );
        let annotations = tool.annotations.unwrap();
        assert_eq!(
            annotations.read_only_hint,
            Some(name.effect() == Effect::Read)
        );
        assert_eq!(
            annotations.open_world_hint,
            Some(matches!(
                name,
                CommandName::StorageCreate | CommandName::StorageReplace | CommandName::StorageTest
            ))
        );
        let schema = Value::Object((*tool.output_schema.unwrap()).clone());
        assert_eq!(schema["type"], "object");
        fn references(value: &Value, root: &Value) {
            match value {
                Value::Object(map) => {
                    if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                        assert!(
                            root.pointer(reference.strip_prefix('#').unwrap()).is_some(),
                            "{reference}"
                        );
                    }
                    for child in map.values() {
                        references(child, root);
                    }
                }
                Value::Array(items) => {
                    for child in items {
                        references(child, root);
                    }
                }
                _ => {}
            }
        }
        references(&schema, &schema);
    }
    assert_eq!(names.len(), 24);
    assert!(
        !names
            .iter()
            .any(|name| name.starts_with("identity") || name.starts_with("history"))
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn stateless_discovery_lists_only_resource_tools(pool: PgPool) {
    let token = owner(&pool).await;
    let response = rpc(&pool, &token, "tools/list", json!({})).await;
    let status = response.status();
    let body = json_body(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["result"]["tools"].as_array().unwrap().len(), 24);
    assert!(!body.to_string().contains(&token));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

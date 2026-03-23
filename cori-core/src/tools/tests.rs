/// Session 02 · 验收测试

#[cfg(test)]
mod tests {
    use crate::tools::{bash::BashTool, ToolRegistry};
    use crate::types::ToolUse;

    fn make_call(name: &str, input: serde_json::Value) -> ToolUse {
        ToolUse {
            id: "toolu_test".into(),
            name: name.into(),
            input,
        }
    }

    #[tokio::test]
    async fn test_bash_tool_echo() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);

        let call = make_call("bash", serde_json::json!({ "command": "echo hello" }));
        let result = registry.dispatch(&call).await.unwrap();

        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_tool_failure_is_ok() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);

        let call = make_call(
            "bash",
            serde_json::json!({ "command": "cat /nonexistent_file_xyz" }),
        );
        let result = registry.dispatch(&call).await;
        assert!(
            result.is_ok(),
            "bash failure should be Ok, got: {:?}",
            result
        );
        assert!(!result.unwrap().content.is_empty());
    }

    #[tokio::test]
    async fn test_unknown_tool_returns_ok() {
        let registry = ToolRegistry::new();
        let call = make_call("unknown_tool", serde_json::json!({}));
        let result = registry.dispatch(&call).await;

        assert!(result.is_ok());
        assert!(result.unwrap().content.contains("unknown"));
    }

    #[test]
    fn test_all_schemas() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);

        let schemas = registry.all_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["name"], "bash");
    }
}

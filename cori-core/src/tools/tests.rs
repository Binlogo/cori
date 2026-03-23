/// Session 02 · 验收测试

#[cfg(test)]
mod tests {
    use crate::tools::{ToolRegistry, bash::BashTool};
    use crate::types::ToolUse;

    fn make_call(name: &str, input: serde_json::Value) -> ToolUse {
        ToolUse {
            id: "toolu_test".into(),
            name: name.into(),
            input,
        }
    }

    /// BashTool 能执行命令并返回输出
    #[test]
    fn test_bash_tool_echo() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);

        let call = make_call("bash", serde_json::json!({ "command": "echo hello" }));
        let result = registry.dispatch(&call).unwrap();

        assert!(result.content.contains("hello"));
    }

    /// BashTool 命令失败时返回 Ok（错误信息作为内容），而不是 Err
    #[test]
    fn test_bash_tool_failure_is_ok() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);

        let call = make_call("bash", serde_json::json!({ "command": "cat /nonexistent_file_xyz" }));
        // 失败的命令不应该让 dispatch 返回 Err
        let result = registry.dispatch(&call);
        assert!(result.is_ok(), "bash failure should be Ok, got: {:?}", result);
        // 但输出里应该有错误信息
        assert!(!result.unwrap().content.is_empty());
    }

    /// 未知工具应该返回 Ok 并携带提示信息（而不是 Err）
    #[test]
    fn test_unknown_tool_returns_ok() {
        let registry = ToolRegistry::new();
        let call = make_call("unknown_tool", serde_json::json!({}));
        let result = registry.dispatch(&call);

        assert!(result.is_ok());
        assert!(result.unwrap().content.contains("unknown"));
    }

    /// all_schemas 应该包含所有已注册工具的 schema
    #[test]
    fn test_all_schemas() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);

        let schemas = registry.all_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["name"], "bash");
    }
}

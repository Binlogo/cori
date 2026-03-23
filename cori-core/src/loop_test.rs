/// Session 01 · 测试用例
///
/// 这些测试是你的"验收标准"。
/// 让所有测试通过 = 练习完成。
///
/// 运行方式：
///   cargo test -p cori-core

#[cfg(test)]
mod tests {
    use crate::{
        loop_::{AgentLoop, EchoExecutor, LlmResponse, MockLlm},
        types::{ToolUse},
    };

    /// 最简场景：LLM 第一轮就 end_turn，不调用任何工具
    #[tokio::test]
    async fn test_direct_answer() {
        let llm = MockLlm::new(vec![LlmResponse {
            stop_reason: "end_turn".into(),
            text: Some("Hello from mock!".into()),
            tool_calls: vec![],
        }]);

        let mut agent = AgentLoop::new(llm, EchoExecutor);
        let result = agent.run("Hi").await.unwrap();

        assert_eq!(result, "Hello from mock!");
    }

    /// 工具调用场景：先 tool_use，再 end_turn
    #[tokio::test]
    async fn test_one_tool_call() {
        let llm = MockLlm::new(vec![
            LlmResponse {
                stop_reason: "tool_use".into(),
                text: None,
                tool_calls: vec![ToolUse {
                    id: "toolu_001".into(),
                    name: "bash".into(),
                    input: serde_json::json!({ "command": "echo hi" }),
                }],
            },
            LlmResponse {
                stop_reason: "end_turn".into(),
                text: Some("Done.".into()),
                tool_calls: vec![],
            },
        ]);

        let mut agent = AgentLoop::new(llm, EchoExecutor);
        let result = agent.run("run something").await.unwrap();

        assert_eq!(result, "Done.");
    }

    /// 安全阀测试：超过 max_turns 应该返回错误
    #[tokio::test]
    async fn test_max_turns_exceeded() {
        // LLM 永远返回 tool_use，永不 end_turn
        let responses: Vec<LlmResponse> = (0..10)
            .map(|i| LlmResponse {
                stop_reason: "tool_use".into(),
                text: None,
                tool_calls: vec![ToolUse {
                    id: format!("toolu_{i:03}"),
                    name: "bash".into(),
                    input: serde_json::json!({}),
                }],
            })
            .collect();

        let mut agent = AgentLoop::new(MockLlm::new(responses), EchoExecutor);
        // TODO: 给 AgentLoop 加一个 with_max_turns() builder 方法，
        //       让这里能写 .with_max_turns(3)
        let result = agent.run("loop forever").await;

        assert!(result.is_err(), "should fail after max turns");
    }
}

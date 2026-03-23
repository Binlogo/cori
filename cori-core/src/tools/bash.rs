use std::process::Command;

/// Session 02 · BashTool
///
/// 最基础的工具：在子进程里执行 shell 命令，返回 stdout + stderr。
///
/// Exercise 1：先读懂 schema()，再实现 execute()。
/// 注意 schema 的格式——这就是 Claude 看到的"工具说明书"。
use super::Tool;

pub struct BashTool;

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    /// 执行 shell 命令。
    ///
    /// `input` 的格式由下方 schema 约定：{ "command": "ls -la" }
    ///
    /// 思考：
    ///   1. 命令超时怎么办？（现在先不处理，记住这个问题）
    ///   2. 命令失败（exit code != 0）时应该返回 Err 还是把错误信息作为 Ok 返回？
    ///      Claude Code 的选择是？
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'command' field"))?;

        // 用 std::process::Command 执行命令
        //
        // 提示：
        //   Command::new("sh").arg("-c").arg(command)
        //
        // 需要合并 stdout 和 stderr，因为 Claude 需要看到完整输出。
        // exit code != 0 时，把 stderr 作为内容返回（而不是 Err），
        // 让 Claude 自己决定怎么处理错误。

        let output = Command::new("sh").arg("-c").arg(command).output()?;

        let mut out = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.stderr.is_empty() {
            out.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        // 失败时加上 exit code，让 Claude 知道命令没成功
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            out.push_str(&format!("\n[exit code: {code}]"));
        }
        Ok(out)
    }

    /// 这个 schema 会被发送给 Claude，告诉它：
    ///   - 工具叫什么（name）
    ///   - 是做什么的（description）
    ///   - 需要哪些参数（input_schema）
    ///
    /// 这是 Anthropic tool use API 规定的格式，不能随意改动字段名。
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "bash",
            "description": "Execute a shell command and return its output. Use for file operations, running programs, and interacting with the system.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute."
                    }
                },
                "required": ["command"]
            }
        })
    }
}

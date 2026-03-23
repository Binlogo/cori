use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use colored::Colorize;
use rustyline::{error::ReadlineError, DefaultEditor};

use cori_core::{
    claude::ClaudeLlm,
    loop_::AgentLoop,
    planner::TaskList,
    tools::{
        bash::BashTool,
        fs::{GlobTool, GrepTool, ReadFileTool, WriteFileTool},
        todo::{TodoReadTool, TodoWriteTool},
        ToolRegistry,
    },
    types::{Message, ToolResult, ToolUse},
};

// ── CLI 工具执行器 ─────────────────────────────────────────────────────────────
//
// 包装 ToolRegistry，在每次工具调用前后打印友好的 UI。

struct CliExecutor {
    inner: ToolRegistry,
}

impl cori_core::loop_::ToolExecutor for CliExecutor {
    async fn execute(&self, call: &ToolUse) -> Result<ToolResult, anyhow::Error> {
        // 工具调用头部
        println!(
            "  {} {}",
            "●".yellow(),
            format!("Tool: {}", call.name).bold()
        );

        // 参数预览（取第一个字段，通常最具代表性）
        let preview = input_preview(&call.input);
        if !preview.is_empty() {
            println!(
                "    {} {}",
                "○".dimmed(),
                format!("Running: {preview}").dimmed()
            );
        }
        io::stdout().flush().ok();

        let result = self.inner.dispatch(call).await?;

        // 结果预览（最多 8 行）
        let lines: Vec<&str> = result.content.lines().collect();
        for line in lines.iter().take(8) {
            println!("    {}", line.dimmed());
        }
        if lines.len() > 8 {
            println!(
                "    {}",
                format!("… {} more lines", lines.len() - 8).dimmed()
            );
        }
        println!();

        Ok(result)
    }
}

/// 提取工具输入的单行摘要（用于 UI 显示）
fn input_preview(input: &serde_json::Value) -> String {
    if let Some(obj) = input.as_object() {
        // 优先取 "command"、"content" 等常见字段
        for key in &["command", "content", "path", "query"] {
            if let Some(v) = obj.get(*key) {
                if let Some(s) = v.as_str() {
                    return truncate(s, 80);
                }
            }
        }
        // 退而求其次：取第一个字段
        if let Some((_, v)) = obj.iter().next() {
            if let Some(s) = v.as_str() {
                return truncate(s, 80);
            }
        }
    }
    truncate(&input.to_string(), 80)
}

fn truncate(s: &str, max: usize) -> String {
    // 按字符边界截断，避免 UTF-8 panic
    let mut end = max.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    if end < s.len() {
        format!("{}…", &s[..end])
    } else {
        s.to_owned()
    }
}

// ── 显示工具函数 ──────────────────────────────────────────────────────────────

fn print_banner() {
    println!();
    println!("  {}", "◆ Cori".cyan().bold());
    println!(
        "  {}",
        "AI Agent CLI  ·  /help 查看命令".truecolor(120, 120, 120)
    );
    println!();
}

fn print_help() {
    println!("{}", "命令列表:".cyan().bold());
    println!("  {}    显示此帮助", "/help".bold());
    println!("  {}   清空对话历史", "/clear".bold());
    println!("  {}    退出 cori", "/exit".bold());
    println!();
}

fn print_response(text: &str) {
    println!();
    for line in text.lines() {
        println!("{line}");
    }
    println!();
}

fn print_error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg.red());
    println!();
}

fn print_separator() {
    println!("{}", "─".repeat(48).truecolor(60, 60, 60));
}

// ── 主程序 ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 只显示 WARN 以上日志，保持 UI 干净
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .without_time()
        .with_target(false)
        .init();

    // 构建工具注册表
    let task_list = Arc::new(Mutex::new(TaskList::load(".cori_tasks.json")?));
    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    registry.register(TodoReadTool::new(Arc::clone(&task_list)));
    registry.register(TodoWriteTool::new(Arc::clone(&task_list)));
    // Session 07：文件系统工具
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(GlobTool);
    registry.register(GrepTool);

    // 先获取 schemas，再把 registry 移入 executor
    let schemas = registry.all_schemas();
    let llm = ClaudeLlm::from_env(schemas)?;
    let executor = CliExecutor { inner: registry };
    let mut agent = AgentLoop::new(llm, executor);

    print_banner();

    // readline 编辑器（支持方向键、历史记录）
    let mut rl = DefaultEditor::new()?;

    // 多轮对话历史：跨用户输入持久保留
    let mut messages: Vec<Message> = vec![];

    loop {
        // 绿色 ❯ 提示符
        let prompt = format!("{} ", "❯".green().bold());
        match rl.readline(&prompt) {
            Ok(line) => {
                let input = line.trim().to_owned();
                if input.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&input);

                // 内置命令
                match input.as_str() {
                    "/exit" | "/quit" => {
                        println!("{}", "Goodbye!".dimmed());
                        break;
                    }
                    "/help" => {
                        println!();
                        print_help();
                        continue;
                    }
                    "/clear" => {
                        messages.clear();
                        println!("{}", "对话历史已清空。".dimmed());
                        println!();
                        continue;
                    }
                    _ => {}
                }

                // 将用户消息加入历史
                messages.push(Message::user(&input));
                println!();

                // 运行 agent（工具调用会在 CliExecutor 中实时打印）
                match agent.run_turn(&mut messages).await {
                    Ok(response) => {
                        // 如果本轮有工具调用，打印分隔线再显示最终回复
                        let has_tool_calls = messages.iter().any(|m| {
                            m.content
                                .iter()
                                .any(|c| matches!(c, cori_core::types::Content::ToolUse(_)))
                        });
                        if has_tool_calls {
                            print_separator();
                        }
                        print_response(&response);
                    }
                    Err(e) => {
                        // 出错时移除未完成的用户消息，避免历史污染
                        messages.pop();
                        print_error(&e.to_string());
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C：提示而不退出
                println!("{}", "\n(用 /exit 退出，或 Ctrl+D)".dimmed());
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D：正常退出
                println!("{}", "Goodbye!".dimmed());
                break;
            }
            Err(e) => {
                eprintln!("readline error: {e}");
                break;
            }
        }
    }

    Ok(())
}

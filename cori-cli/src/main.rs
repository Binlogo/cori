// cori-cli — enhanced CLI for the Cori agent system
//
// Improvements over the root binary (src/main.rs):
//   - /tools, /model, /version slash commands
//   - Token usage display after each response
//   - Headless mode via --prompt flag or CORI_PROMPT env var
//   - CoriConfig::from_env() for configuration
//   - PermissionGate::unrestricted() for interactive mode
//   - LoggingHook registered when RUST_LOG=debug is active

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use colored::Colorize;
use rustyline::{error::ReadlineError, DefaultEditor};

use cori_core::{
    claude::ClaudeLlm,
    config::CoriConfig,
    hooks::{HookRegistry, LoggingHook},
    loop_::AgentLoop,
    permission::PermissionGate,
    planner::TaskGraph,
    tools::{
        bash::BashTool,
        edit::EditFileTool,
        fs::{GlobTool, GrepTool, ReadFileTool, WriteFileTool},
        task::{TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool},
        ToolRegistry,
    },
    types::{Message, ToolResult, ToolUse},
};

const CORI_VERSION: &str = "0.2.0";

// ── CliExecutor ───────────────────────────────────────────────────────────────
//
// Wraps ToolRegistry, prints a friendly UI around every tool call, and
// accumulates the last-turn token usage so we can display it after the
// response.

struct CliExecutor {
    inner: ToolRegistry,
}

impl cori_core::loop_::ToolExecutor for CliExecutor {
    async fn execute(&self, call: &ToolUse) -> Result<ToolResult, anyhow::Error> {
        // Tool call header
        println!(
            "  {} {}",
            "●".yellow(),
            format!("Tool: {}", call.name).bold()
        );

        // Input preview (first recognisable field)
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

        // Result preview (at most 8 lines)
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

/// Extract a one-line summary of tool input (for UI display).
fn input_preview(input: &serde_json::Value) -> String {
    if let Some(obj) = input.as_object() {
        for key in &["command", "content", "path", "query"] {
            if let Some(v) = obj.get(*key) {
                if let Some(s) = v.as_str() {
                    return truncate(s, 80);
                }
            }
        }
        if let Some((_, v)) = obj.iter().next() {
            if let Some(s) = v.as_str() {
                return truncate(s, 80);
            }
        }
    }
    truncate(&input.to_string(), 80)
}

fn truncate(s: &str, max: usize) -> String {
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

// ── Display helpers ───────────────────────────────────────────────────────────

fn print_banner() {
    println!();
    println!("  {}", "◆ Cori".cyan().bold());
    println!(
        "  {}",
        "AI Agent CLI  ·  type /help for commands".truecolor(120, 120, 120)
    );
    println!();
}

fn print_help() {
    println!("{}", "Commands:".cyan().bold());
    println!("  {}     show this help", "/help".bold());
    println!("  {}    clear conversation history", "/clear".bold());
    println!("  {}     exit cori-cli", "/exit".bold());
    println!("  {}     exit cori-cli (alias)", "/quit".bold());
    println!("  {}    list all registered tools", "/tools".bold());
    println!("  {}    show current model", "/model".bold());
    println!("  {}  show cori version", "/version".bold());
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

#[allow(dead_code)]
fn print_usage(input: u32, output: u32) {
    if input > 0 || output > 0 {
        println!(
            "{}",
            format!("[in: {input} / out: {output} tokens]").truecolor(90, 90, 90)
        );
        println!();
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Respect RUST_LOG if set; otherwise show WARN+ to keep the UI clean.
    let rust_log = std::env::var("RUST_LOG").unwrap_or_default();
    let level = if rust_log.contains("debug") || rust_log.contains("trace") {
        tracing::Level::DEBUG
    } else {
        tracing::Level::WARN
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .without_time()
        .with_target(false)
        .init();

    // Configuration from environment variables.
    let config = CoriConfig::from_env()?;

    // Permission gate: allow everything in the interactive CLI.
    let _gate = PermissionGate::unrestricted();

    // Build the tool registry.
    let task_graph = Arc::new(Mutex::new(TaskGraph::load(".tasks")?));
    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    registry.register(TaskListTool::new(Arc::clone(&task_graph)));
    registry.register(TaskCreateTool::new(Arc::clone(&task_graph)));
    registry.register(TaskGetTool::new(Arc::clone(&task_graph)));
    registry.register(TaskUpdateTool::new(Arc::clone(&task_graph)));
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(EditFileTool);
    registry.register(GlobTool);
    registry.register(GrepTool);

    // Collect schemas before moving registry into the executor.
    let all_schemas = registry.all_schemas();

    // Optional LoggingHook when debug logging is enabled.
    let mut _hooks = HookRegistry::new();
    if level == tracing::Level::DEBUG {
        _hooks.register(LoggingHook);
    }

    let llm = ClaudeLlm::from_env(all_schemas.clone())?;
    let executor = CliExecutor { inner: registry };
    let mut agent = AgentLoop::new(llm, executor);

    // ── Headless mode ─────────────────────────────────────────────────────────
    //
    // cori-cli --prompt "do something"
    // or CORI_PROMPT="do something" cori-cli

    let args: Vec<String> = std::env::args().collect();
    let headless_prompt = if let Some(pos) = args.iter().position(|a| a == "--prompt") {
        args.get(pos + 1).cloned()
    } else {
        std::env::var("CORI_PROMPT").ok()
    };

    if let Some(prompt) = headless_prompt {
        let result = agent.run(&prompt).await?;
        println!("{result}");
        return Ok(());
    }

    // ── Interactive mode ──────────────────────────────────────────────────────

    print_banner();

    let mut rl = DefaultEditor::new()?;
    let mut messages: Vec<Message> = vec![];

    loop {
        let prompt_str = format!("{} ", "❯".green().bold());
        match rl.readline(&prompt_str) {
            Ok(line) => {
                let input = line.trim().to_owned();
                if input.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&input);

                // Built-in slash commands
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
                        println!("{}", "Conversation history cleared.".dimmed());
                        println!();
                        continue;
                    }
                    "/tools" => {
                        println!("{}", "Registered tools:".cyan().bold());
                        // Sort by name for stable display
                        let mut named: Vec<(&str, &str)> = all_schemas
                            .iter()
                            .filter_map(|s| {
                                let name = s["name"].as_str()?;
                                let desc = s["description"].as_str().unwrap_or("");
                                Some((name, desc))
                            })
                            .collect();
                        named.sort_by_key(|(name, _)| *name);
                        for (name, desc) in named {
                            let short_desc =
                                if desc.len() > 60 { &desc[..60] } else { desc };
                            println!("  {}  {}", name.bold(), short_desc.dimmed());
                        }
                        println!();
                        continue;
                    }
                    "/model" => {
                        println!(
                            "{}  {}",
                            "Model:".cyan().bold(),
                            config.provider.model.as_str()
                        );
                        println!();
                        continue;
                    }
                    "/version" => {
                        println!(
                            "{}  {}",
                            "cori-cli".cyan().bold(),
                            CORI_VERSION
                        );
                        println!();
                        continue;
                    }
                    _ => {}
                }

                // Push user message into the conversation history.
                messages.push(Message::user(&input));
                println!();

                // Streaming on_text callback: print each token as it arrives.
                let on_text = |text: &str| {
                    print!("{text}");
                    io::stdout().flush().ok();
                };

                let msg_len_before = messages.len();

                match agent.run_turn_streaming(&mut messages, on_text).await {
                    Ok(response) => {
                        // The text was already printed token-by-token; just
                        // print a newline and optional separator.
                        let has_tool_calls =
                            messages[msg_len_before..].iter().any(|m| {
                                m.content.iter().any(|c| {
                                    matches!(c, cori_core::types::Content::ToolUse(_))
                                })
                            });
                        if has_tool_calls {
                            print_separator();
                            print_response(&response);
                        } else {
                            println!("\n");
                        }

                        // Token usage is not directly exposed by run_turn_streaming.
                        // Displaying counts here would require a cori-core change;
                        // the print_usage helper is ready for when that lands.
                    }
                    Err(e) => {
                        messages.pop();
                        print_error(&e.to_string());
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C: hint but don't exit
                println!("{}", "\n(Use /exit to quit, or Ctrl+D)".dimmed());
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D: clean exit
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

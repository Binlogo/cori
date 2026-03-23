/// Session 07 · File System Tools
///
/// 这四个工具是 coding agent 的"感觉器官"：
///   - read_file  → 读取文件内容（眼睛）
///   - write_file → 写入 / 修改文件（手）
///   - glob       → 按通配符查找文件（地图）
///   - grep       → 在文件里搜索字符串（搜索引擎）
///
/// Claude Code 的核心能力正是来自这四类工具的组合。
/// 有了它们，Agent 就能阅读代码、理解结构、然后有针对性地修改。
///
/// 本节练习的重点不只是"实现"，更是理解每个工具的设计权衡：
///   - 行号为什么从 1 开始？
///   - write_file 为什么要整体覆盖而不是追加？
///   - grep 跳过 target/ 是硬编码对吗？

use std::{fs, path::Path};

use super::Tool;

// ── ReadFileTool ──────────────────────────────────────────────────────────────

pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    /// Exercise 1：读取文件内容，可选指定行范围。
    ///
    /// 步骤：
    ///   1. 从 input["path"] 取路径字符串（缺少时返回 Err）
    ///   2. std::fs::read_to_string 读取文件；文件不存在时给出含路径的错误
    ///   3. 按 start_line / end_line 切片（行号从 1 开始，两端均含）
    ///      - 若未指定，读全文
    ///   4. 给每行加行号前缀："{n:>4} │ {line}\n"
    ///   5. 空文件返回 "(empty file)"
    ///
    /// 提示：
    ///   let lines: Vec<&str> = content.lines().collect();
    ///   lines[from..to].iter().enumerate().map(|(i, line)| format!(...))
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path' field"))?;

        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read {path}: {e}"))?;

        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();

        if total == 0 {
            return Ok("(empty file)".into());
        }

        // 行号从 1 开始；saturating_sub 避免 start_line=0 时下溢
        let from = input["start_line"]
            .as_u64()
            .map(|n| (n as usize).saturating_sub(1))
            .unwrap_or(0);
        let to = input["end_line"]
            .as_u64()
            .map(|n| (n as usize).min(total))
            .unwrap_or(total);

        // Exercise 1：把 lines[from..to] 拼成带行号的字符串
        //
        //   for (i, line) in lines[from..to].iter().enumerate() {
        //       out.push_str(&format!("{:>4} │ {}\n", from + i + 1, line));
        //   }
        //
        // 把上面的注释取消，删掉下面这行，即完成练习。

        anyhow::bail!("Exercise 1: implement read_file (see comments above)")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "read_file",
            "description": "Read the contents of a file. Returns content with line numbers. \
                           Optionally specify start_line/end_line to read a range.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read."
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "First line to read (1-indexed, inclusive). Defaults to 1."
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line to read (1-indexed, inclusive). Defaults to end of file."
                    }
                },
                "required": ["path"]
            }
        })
    }
}

// ── WriteFileTool ─────────────────────────────────────────────────────────────

pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    /// Exercise 2：把 content 写入 path，覆盖已有内容。
    ///
    /// 步骤：
    ///   1. 从 input["path"] 和 input["content"] 取值
    ///   2. 如果父目录不存在，先创建（create_dir_all）
    ///      Path::new(path).parent() 返回 Option<&Path>
    ///   3. fs::write(path, content)?
    ///   4. 返回 "Written N bytes to {path}."（N = content.len()）
    ///
    /// 思考：write_file 为什么整体覆盖而不是追加？
    ///   Claude 每次发来的是"新版全文"，这样它不需要关心当前文件的状态。
    ///   追加模式会让 Claude 必须知道文件现在长什么样。
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path' field"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'content' field"))?;

        // Exercise 2：创建父目录（如不存在），然后写入文件
        //
        //   if let Some(parent) = Path::new(path).parent() {
        //       if !parent.as_os_str().is_empty() {
        //           fs::create_dir_all(parent)?;
        //       }
        //   }
        //   fs::write(path, content)?;
        //   Ok(format!("Written {} bytes to {path}.", content.len()))
        //
        // 把注释取消，删掉下面这行，即完成练习。

        anyhow::bail!("Exercise 2: implement write_file (see comments above)")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "write_file",
            "description": "Write content to a file, creating it and any parent directories \
                           if they don't exist. Overwrites existing content.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write."
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file."
                    }
                },
                "required": ["path", "content"]
            }
        })
    }
}

// ── GlobTool ──────────────────────────────────────────────────────────────────

pub struct GlobTool;

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    /// Exercise 3：按通配符模式查找文件。
    ///
    /// 步骤：
    ///   1. 从 input["pattern"] 取 glob 模式，如 "src/**/*.rs"
    ///   2. 用 glob::glob(pattern) 遍历所有匹配项
    ///   3. 过滤出文件（is_file()），排除目录
    ///   4. 排序后以换行符拼接路径
    ///   5. 没有匹配时返回 "No files found."
    ///
    /// 提示：
    ///   for entry in glob::glob(pattern)? {
    ///       if let Ok(path) = entry {
    ///           if path.is_file() { results.push(path.display().to_string()); }
    ///       }
    ///   }
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern' field"))?;

        // Exercise 3：用 glob::glob 遍历，收集文件路径并排序
        //
        //   let mut paths: Vec<String> = vec![];
        //   for entry in glob::glob(pattern)? {
        //       if let Ok(path) = entry {
        //           if path.is_file() {
        //               paths.push(path.display().to_string());
        //           }
        //       }
        //   }
        //   paths.sort();
        //   if paths.is_empty() {
        //       return Ok("No files found.".into());
        //   }
        //   Ok(paths.join("\n"))
        //
        // 把注释取消，删掉下面这行，即完成练习。

        anyhow::bail!("Exercise 3: implement glob (see comments above)")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "glob",
            "description": "Find files matching a glob pattern. \
                           Use ** for recursive matching, e.g. 'src/**/*.rs'.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to match files against, e.g. '**/*.rs', 'src/**/*.toml'."
                    }
                },
                "required": ["pattern"]
            }
        })
    }
}

// ── GrepTool ──────────────────────────────────────────────────────────────────

pub struct GrepTool;

/// 在 dir 下递归搜索包含 pattern 的行，结果追加到 results。
///
/// 跳过规则（硬编码）：
///   - 以 "." 开头的目录（.git, .cargo 等）
///   - "target" 目录（Rust 构建产物）
///
/// 思考：这里硬编码 target/ 合理吗？
///   对于 Cori 这个 Rust 项目，合理。但如果要做通用工具，
///   应该让 Claude 传一个 ignore 列表过来。
fn grep_recursive(
    dir: &Path,
    pattern: &str,
    ext_filter: Option<&str>,
    results: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(dir) else { return };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name.starts_with('.') || name == "target" {
                continue;
            }
            grep_recursive(&path, pattern, ext_filter, results);
        } else {
            // 文件扩展名过滤
            if let Some(ext) = ext_filter {
                let file_ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if file_ext != ext {
                    continue;
                }
            }

            // 读取并搜索（跳过非 UTF-8 二进制文件）
            if let Ok(content) = fs::read_to_string(&path) {
                for (i, line) in content.lines().enumerate() {
                    if line.contains(pattern) {
                        results.push(format!(
                            "{}:{}: {}",
                            path.display(),
                            i + 1,
                            line.trim()
                        ));
                    }
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    /// Exercise 4：在目录下递归搜索包含 pattern 的行。
    ///
    /// 步骤：
    ///   1. 从 input["pattern"] 取搜索字符串
    ///   2. 从 input["path"] 取搜索目录（默认 "."）
    ///   3. 从 input["glob"] 取文件扩展名过滤（如 "rs"，可选）
    ///      注意：这里为了简单，glob 只支持 "*.ext" 格式，
    ///            实现时只取扩展名部分（strip_prefix("*.")）
    ///   4. 调用 grep_recursive 收集结果
    ///   5. 最多返回 50 条，超出时追加 "... N more matches"
    ///   6. 没有匹配时返回 "No matches found."
    ///
    /// 提示：
    ///   let mut results = vec![];
    ///   grep_recursive(Path::new(dir), pattern, ext_filter, &mut results);
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern' field"))?;
        let dir = input["path"].as_str().unwrap_or(".");

        // 把 "*.rs" 转成 ext_filter = Some("rs")
        let ext_filter: Option<&str> = input["glob"]
            .as_str()
            .and_then(|g| g.strip_prefix("*."));

        // Exercise 4：调用 grep_recursive，处理结果数量限制
        //
        //   let mut results: Vec<String> = vec![];
        //   grep_recursive(Path::new(dir), pattern, ext_filter, &mut results);
        //   results.sort();
        //
        //   if results.is_empty() {
        //       return Ok("No matches found.".into());
        //   }
        //
        //   const MAX: usize = 50;
        //   let mut out = results[..results.len().min(MAX)].join("\n");
        //   if results.len() > MAX {
        //       out.push_str(&format!("\n... {} more matches", results.len() - MAX));
        //   }
        //   Ok(out)
        //
        // 把注释取消，删掉下面这行，即完成练习。

        anyhow::bail!("Exercise 4: implement grep (see comments above)")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "grep",
            "description": "Search for a string pattern in files under a directory. \
                           Returns matching lines with file:line format. \
                           Skips binary files and build artifacts (target/).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "String to search for (case-sensitive)."
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in. Defaults to current directory."
                    },
                    "glob": {
                        "type": "string",
                        "description": "File extension filter, e.g. '*.rs' to only search Rust files."
                    }
                },
                "required": ["pattern"]
            }
        })
    }
}

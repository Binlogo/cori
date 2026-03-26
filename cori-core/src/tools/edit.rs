/// Session 09 · Edit Tool
///
/// EditFileTool：Claude Code 最核心的工具之一。
///
/// 为什么不直接用 WriteFileTool？
///   write_file 需要发来"新版全文"。文件越长，token 消耗越多，
///   而且 Claude 重写整个文件时容易无意中修改其他部分。
///   edit_file 只需发来"要改的那段"，既省 token，又精准，风险更小。
///
/// 唯一性约束：
///   old_string 必须在文件中恰好出现一次。
///
///   出现 0 次 → Claude 需要先读文件确认内容（可能文件已被其他工具修改）
///   出现 2+ 次 → Claude 需要提供更多上下文使其唯一（加上前后几行）
///   出现 1 次 → 精确替换 ✓
///
/// 这个设计使 edit_file 具备"事务性"：
///   要么替换成功，要么拒绝操作——不会在错误的位置悄悄改动。
use std::fs;

use super::Tool;

pub struct EditFileTool;

#[async_trait::async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    /// 步骤（顺序实现 Exercise 1 → Exercise 2）：
    ///   1. 从 input 取 path / old_string / new_string（均必填）
    ///   2. 读取文件内容（文件不存在时返回含路径的错误）
    ///   3. 统计 old_string 在内容中出现的次数
    ///   4. 根据次数决定操作（见 Exercise 1）
    ///   5. 替换后写回文件
    ///   6. 返回 diff 风格的输出（见 Exercise 2）
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path' field"))?;
        let old_string = input["old_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'old_string' field"))?;
        let new_string = input["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'new_string' field"))?;

        let content =
            fs::read_to_string(path).map_err(|e| anyhow::anyhow!("cannot read {path}: {e}"))?;

        // Exercise 1：统计出现次数，按结果分支处理
        //
        // 提示：
        //   let count = content.matches(old_string).count();
        //   match count {
        //       0 => anyhow::bail!("old_string not found in {path}"),
        //       1 => { /* 替换 */ }
        //       n => anyhow::bail!("ambiguous: {n} occurrences in {path}, add more context"),
        //   }
        //
        // 替换：
        //   let new_content = content.replacen(old_string, new_string, 1);
        //   fs::write(path, &new_content)?;
        let count = content.matches(old_string).count();
        match count {
            0 => anyhow::bail!("old_string not found in {path}"),
            1 => {
                let new_content = content.replacen(old_string, new_string, 1);
                fs::write(path, new_content)?;
            }
            n => anyhow::bail!("ambiguous: {n} occurrences in {path}, add more context"),
        }

        // Exercise 2：生成 diff 风格的输出
        //
        // 直接返回 "Replaced in {path}" 太简陋——Claude 需要看到"改了什么"
        // 才能验证操作是否符合预期，并在下一轮调用中引用正确的上下文。
        //
        // 格式：
        //   Edited path/to/file
        //
        //   @@ line N @@
        //    context line      (空格前缀 = 未变)
        //   -old content       (减号前缀 = 删除)
        //   +new content       (加号前缀 = 新增)
        //    context line
        //
        // Exercise 2 — 把 "Replaced." 换成 make_diff(...) 的返回值

        Ok(make_diff(path, &content, old_string, new_string, 2))
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "edit_file",
            "description": "Replace an exact string in a file. \
                           old_string must appear exactly once — if it appears 0 or 2+ times \
                           the edit is rejected. Provide enough context lines to make it unique.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to edit."
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact string to find and replace. \
                                       Must appear exactly once in the file."
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The string to replace old_string with."
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }
        })
    }
}

// ── Exercise 2：make_diff ──────────────────────────────────────────────────────

/// 生成 diff 风格的字符串，展示替换前后的变化。
///
/// 参数：
///   path        — 文件路径（用于显示）
///   content     — 替换前的原始内容
///   old_string  — 被替换的旧字符串
///   new_string  — 替换成的新字符串
///   context     — 上下文行数（建议 2）
///
/// 步骤：
///   1. 找到 old_string 在 content 中的字节偏移，换算成行号（find_match_line）
///   2. 计算上下文范围：[start_line - context, end_line + context]
///   3. 输出每行，前缀规则：
///      - 上下文行：" "（空格）
///      - 删除行：  "-"
///      - 新增行：  "+"
///
/// 提示：
///   let all_lines: Vec<&str> = content.lines().collect();
///   let old_lines: Vec<&str> = old_string.lines().collect();
///   let new_lines: Vec<&str> = new_string.lines().collect();
///   let match_start = find_match_line(content, old_string); // 0-indexed
///   let match_end = match_start + old_lines.len();
pub(crate) fn make_diff(
    path: &str,
    content: &str,
    old_string: &str,
    new_string: &str,
    context: usize,
) -> String {
    let all_lines: Vec<&str> = content.lines().collect();
    let old_lines: Vec<&str> = old_string.lines().collect();
    let new_lines: Vec<&str> = new_string.lines().collect();

    let match_start = find_match_line(content, old_string);
    let match_end = match_start + old_lines.len();
    let ctx_start = match_start.saturating_sub(context);
    let ctx_end = (match_end + context).min(all_lines.len());

    let mut out = format!("Edited {path}\n\n@@ line {} @@\n", match_start + 1);
    for i in ctx_start..match_start {
        out.push_str(&format!(" {}\n", all_lines[i]));
    }
    for line in &old_lines {
        out.push_str(&format!("-{line}\n"));
    }
    for line in &new_lines {
        out.push_str(&format!("+{line}\n"));
    }
    for i in match_end..ctx_end {
        out.push_str(&format!(" {}\n", all_lines[i]));
    }
    out
}

/// 找到 old_string 在 content 中首次出现的行号（0-indexed）。
///
/// 原理：在 old_string 出现位置之前的内容里数有几个 '\n'，
///       就是行号（0-indexed）。
pub(crate) fn find_match_line(content: &str, old_string: &str) -> usize {
    let pos = content.find(old_string).unwrap_or(0);
    content[..pos].chars().filter(|&c| c == '\n').count()
}

// ── 测试 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_input(path: &str, old: &str, new: &str) -> serde_json::Value {
        serde_json::json!({
            "path": path,
            "old_string": old,
            "new_string": new
        })
    }

    // Exercise 1 的验收测试
    // 先跑 `cargo test`，看到 FAILED，再实现，再跑到 PASSED。

    #[tokio::test]
    async fn test_exact_replacement() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "fn main() {{\n    println!(\"hello\");\n}}\n").unwrap();

        let result = EditFileTool
            .execute(&make_input(
                f.path().to_str().unwrap(),
                "println!(\"hello\")",
                "println!(\"world\")",
            ))
            .await
            .unwrap();

        // 替换后文件内容已更新
        let content = fs::read_to_string(f.path()).unwrap();
        assert!(
            content.contains("println!(\"world\")"),
            "file should be updated"
        );
        assert!(
            !content.contains("println!(\"hello\")"),
            "old string should be gone"
        );
        // 返回值包含路径
        assert!(
            result.contains(f.path().to_str().unwrap())
                || result.contains("Replaced")
                || result.contains("Edited")
        );
    }

    #[tokio::test]
    async fn test_not_found_returns_error() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "fn main() {{}}\n").unwrap();

        let result = EditFileTool
            .execute(&make_input(
                f.path().to_str().unwrap(),
                "nonexistent string xyz",
                "replacement",
            ))
            .await;

        assert!(result.is_err(), "should error when old_string not found");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not found") || msg.contains("0"),
            "error: {msg}"
        );
    }

    #[tokio::test]
    async fn test_ambiguous_returns_error() {
        let mut f = NamedTempFile::new().unwrap();
        // "foo" 出现两次
        write!(f, "foo\nbar\nfoo\n").unwrap();

        let result = EditFileTool
            .execute(&make_input(f.path().to_str().unwrap(), "foo", "baz"))
            .await;

        assert!(
            result.is_err(),
            "should error when old_string appears 2+ times"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ambiguous") || msg.contains("2"),
            "error: {msg}"
        );
    }

    // Exercise 2 的验收测试
    #[test]
    fn test_find_match_line() {
        let content = "line1\nline2\nline3\nline4\n";
        assert_eq!(find_match_line(content, "line1"), 0);
        assert_eq!(find_match_line(content, "line3"), 2);
    }

    #[test]
    fn test_make_diff_contains_markers() {
        let content = "line1\nfoo\nline3\n";
        let diff = make_diff("test.rs", content, "foo", "bar", 1);
        // 实现后，diff 应该包含 "-foo" 和 "+bar"
        if !diff.is_empty() {
            assert!(diff.contains("-foo"), "diff: {diff}");
            assert!(diff.contains("+bar"), "diff: {diff}");
        }
    }
}

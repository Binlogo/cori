# Session 07 · File System Tools

> **Motto**: Code is just files. Files are just text.

---

## 概念先行

到目前为止，我们的 Agent 只能执行 bash 命令。
这已经很强大，但有个问题：Claude 看不懂 bash 输出的"结构"。

```
bash("find src -name '*.rs'")
→ "src/main.rs\nsrc/lib.rs\n..."   ← 纯文本，没有行号，没有上下文
```

文件系统工具的价值在于：
- **read_file** 返回带行号的内容，让 Claude 能精确引用 "第 42 行"
- **glob** 返回结构化的路径列表，便于批量处理
- **grep** 直接给出 file:line 格式，Claude 可以立刻 read_file 查看上下文
- **write_file** 整体覆盖，Claude 不需要知道当前文件状态

这四个工具组合，就是 Claude Code 读写代码的核心机制。

---

## 工具设计对比

| 工具 | bash 等价 | 为什么要专门封装？ |
|------|----------|------------------|
| read_file | `cat -n` | 带行号、支持行范围、返回结构化内容 |
| write_file | `cat >` | 自动创建目录、明确的整体覆盖语义 |
| glob | `find` | 输出格式稳定，不受 shell 环境影响 |
| grep | `grep -rn` | 自动跳过 target/、返回格式固定 |

---

## 练习 1 — 实现 `ReadFileTool::execute()`

打开 `cori-core/src/tools/fs.rs`，找到 `ReadFileTool::execute()`。

核心代码（注释里已经写好）：

```rust
let lines: Vec<&str> = content.lines().collect();
for (i, line) in lines[from..to].iter().enumerate() {
    out.push_str(&format!("{:>4} │ {}\n", from + i + 1, line));
}
```

**问题**：行号为什么从 1 开始而不是 0？

---

## 练习 2 — 实现 `WriteFileTool::execute()`

核心代码：

```rust
if let Some(parent) = Path::new(path).parent() {
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)?;
    }
}
fs::write(path, content)?;
Ok(format!("Written {} bytes to {path}.", content.len()))
```

**问题**：为什么用整体覆盖而不是 "编辑特定行"？

---

## 练习 3 — 实现 `GlobTool::execute()`

使用 `glob` crate：

```rust
for entry in glob::glob(pattern)? {
    if let Ok(path) = entry {
        if path.is_file() {
            paths.push(path.display().to_string());
        }
    }
}
```

**问题**：`**/*.rs` 和 `*.rs` 有什么区别？

---

## 练习 4 — 实现 `GrepTool::execute()`

调用已写好的 `grep_recursive` 辅助函数：

```rust
let mut results: Vec<String> = vec![];
grep_recursive(Path::new(dir), pattern, ext_filter, &mut results);
results.sort();
```

**问题**：`grep_recursive` 为什么硬编码跳过 `target/`？这个设计有什么缺点？

---

## 练习完成后：注册到 CLI

打开 `src/main.rs`，把新工具注册进 CLI：

```rust
use cori_core::tools::fs::{GlobTool, GrepTool, ReadFileTool, WriteFileTool};

registry.register(ReadFileTool);
registry.register(WriteFileTool);
registry.register(GlobTool);
registry.register(GrepTool);
```

然后运行 CLI，问 Cori：
```
❯ 用 glob 找出项目中所有 .rs 文件，然后统计每个文件的行数
```

---

## 验证

```bash
cargo test -p cori-core
cargo run -p cori-core --example hello
```

能回答：
- [ ] `read_file` 和 `bash("cat file")` 的区别是什么？
- [ ] 为什么 `write_file` 要整体覆盖而不是增量编辑？
- [ ] `grep_recursive` 跳过哪些目录？为什么？
- [ ] Claude 如何利用 file:line 格式做精确修改？

---

## 延伸思考

**edit_file 工具（Claude Code 的精华）**：

真正的 coding agent 不用 `write_file`（整体覆盖），而是用 `edit_file`（精确替换）：

```json
{
  "path": "src/main.rs",
  "old_string": "println!(\"Hello\");",
  "new_string": "println!(\"Hello, Cori!\");"
}
```

这样 Claude 只需要发送"差异"，不需要重新发送整个文件。
这是 Claude Code 中 `Edit` 工具的核心思路。

---

## 下一课

[Session 08 · Streaming →](/lessons/08-streaming) *(coming soon)*

# Session 09 · Edit Tool

> **Motto**: 手术刀比铁锹更安全。

---

## 为什么需要 Edit Tool？

Session 07 我们有了 `write_file`，它可以写入任意内容。看起来够用了——那为什么 Claude Code 还需要 `edit_file`？

**问题一：token 消耗**

假设要修改一个 500 行的文件里的一行注释。
- `write_file`：Claude 必须重新生成全部 500 行（消耗大量 output token）
- `edit_file`：Claude 只需发来"要改的那段 + 改成什么"（几十 token）

**问题二：意外修改**

当 Claude 重写整个文件时，可能无意中：
- 改变了其他地方的格式
- 删掉了它没有读到的注释
- 引入了细微的变化

`edit_file` 只动指定的那段，其他位置完全不变。

---

## 唯一性约束

`edit_file` 的核心规则：`old_string` 必须在文件中恰好出现**一次**。

```
出现 0 次 → 报错："old_string not found"
出现 2+ 次 → 报错："ambiguous: 2 occurrences"
出现 1 次 → 替换 ✓
```

这个限制看起来严格，实则是保护机制：

- **0 次**：文件可能已被修改，Claude 需要重新读取确认。
- **2+ 次**：Claude 提供的上下文不够唯一，它需要加上更多周围代码（前后几行）来定位。

结果：Claude 被迫在每次 edit 前确认上下文，大幅减少误操作。

---

## 代码位置

```
cori-core/src/tools/edit.rs   ← 本节练习
```

---

## 练习 1 — 实现唯一性检查

打开 `edit.rs`，找到 `// TODO: Exercise 1` 的注释块，实现它：

```rust
let count = content.matches(old_string).count();
match count {
    0 => anyhow::bail!("old_string not found in {path}"),
    1 => {
        let new_content = content.replacen(old_string, new_string, 1);
        fs::write(path, &new_content)?;
    }
    n => anyhow::bail!("ambiguous: {n} occurrences in {path}, add more context"),
}
```

记得删掉占位符那两行：
```rust
let _ = (&content, old_string, new_string); // 删这行
let new_content = content.clone();           // 删这行
```

**验证**：

```bash
cargo test -p cori-core tools::edit
```

三个测试应该都变成 PASSED：
- `test_exact_replacement` ✓
- `test_not_found_returns_error` ✓
- `test_ambiguous_returns_error` ✓

**问题**：`content.replacen(old_string, new_string, 1)` 和 `content.replace(old_string, new_string)` 有什么区别？既然我们已经确认只有一次出现，用哪个都行——为什么还是选 `replacen`？

---

## 练习 2 — Diff 风格输出

现在 `execute()` 返回 `"Replaced in {path}."`。这对 Claude 来说信息太少——它需要看到改了什么，才能：
1. 验证操作是否符合预期
2. 在下一轮引用正确的代码行号

实现 `make_diff()` 函数（函数签名已在 `edit.rs` 里），格式如下：

```
Edited src/main.rs

@@ line 12 @@
 fn main() {
-    println!("hello");
+    println!("world");
 }
```

实现要点（注释里已有完整代码提示）：

```rust
pub(crate) fn make_diff(path, content, old_string, new_string, context) -> String {
    let all_lines: Vec<&str> = content.lines().collect();
    let old_lines: Vec<&str> = old_string.lines().collect();
    let new_lines: Vec<&str> = new_string.lines().collect();

    let match_start = find_match_line(content, old_string); // 0-indexed
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
```

然后在 `execute()` 里把最后的 `Ok(...)` 改为：

```rust
Ok(make_diff(path, &content, old_string, new_string, 2))
```

记得删掉占位符 `let _ = &new_content;`。

**验证**：

```bash
cargo test -p cori-core tools::edit::tests::test_make_diff_contains_markers
```

---

## 验收

```bash
cargo test -p cori-core   # 所有 19 个测试通过
cargo build               # 编译成功
```

运行 Cori，让它编辑一个文件：

```bash
ANTHROPIC_API_KEY=sk-... cargo run

❯ 在当前目录创建一个 hello.txt，内容是 "hello world"，然后把 world 改成 Rust
```

观察：
1. Claude 先调用 `write_file` 创建文件
2. 再调用 `edit_file` 精确替换（而不是重新 write_file）

---

## 思考题

**Q1：为什么 `old_string` 可以跨多行？**

单行匹配无法应对函数签名跨行、注释块、多行表达式等情况。
跨行 `old_string` 让 Claude 可以用"整个函数头"作为锚点，而不仅仅是某一行。

**Q2：如果文件是二进制的（图片、编译产物），`read_to_string` 会怎样？**

会返回 `Err`，`?` 会把错误传递出去，`execute()` 返回 `Err`，
`ToolRegistry` 把错误信息作为 tool result 告诉 Claude——Claude 会理解并换一种方式。

---

## 延伸：空白符归一化

真实的 Claude Code Edit Tool 有一个回退策略：
当精确匹配失败时，尝试**归一化行首空白**后再匹配。

```rust
// 把每行的行首空白去掉后比较
fn normalize_indent(s: &str) -> String {
    s.lines().map(|l| l.trim_start()).collect::<Vec<_>>().join("\n")
}
```

为什么需要这个？
因为 Claude 在生成 `old_string` 时可能产生轻微的缩进偏差（比如把 4 空格写成 2 空格）。
归一化让工具更健壮，代价是可能在极端情况下匹配到错误位置——所以归一化后仍然要检查唯一性。

---

## 下一课

[Session 10 · Prompt Caching →](/lessons/10-prompt-caching) *(coming soon)*

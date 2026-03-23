use std::sync::{Arc, Mutex};

use cori_core::{
    claude::ClaudeLlm,
    loop_::AgentLoop,
    planner::TaskList,
    tools::{
        ToolRegistry,
        bash::BashTool,
        todo::{TodoReadTool, TodoWriteTool},
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 任务列表持久化到当前目录，重启后状态保留
    let task_list = Arc::new(Mutex::new(TaskList::load(".cori_tasks.json")?));

    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    registry.register(TodoReadTool::new(Arc::clone(&task_list)));
    registry.register(TodoWriteTool::new(Arc::clone(&task_list)));

    let llm = ClaudeLlm::from_env(registry.all_schemas())?;
    let mut agent = AgentLoop::new(llm, registry);

    // 给 Claude 一个需要多步完成的任务，观察它是否主动调用 TodoWrite 规划
    let answer = agent
        .run(
            "请帮我了解一下当前项目的结构：\
            1. 列出根目录的文件和文件夹 \
            2. 查看 Cargo.toml 的内容 \
            3. 统计 src 目录下有多少个 .rs 文件 \
            请先用 todo_write 列出你的计划，再逐步执行。",
        )
        .await?;

    println!("{answer}");

    // 打印最终任务列表状态
    println!("\n── 任务列表 ──");
    println!("{}", task_list.lock().unwrap().display());

    Ok(())
}

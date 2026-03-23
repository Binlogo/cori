use std::sync::{Arc, Mutex};

use cori_core::{
    claude::ClaudeLlm,
    loop_::AgentLoop,
    planner::TaskList,
    tools::{
        ToolRegistry,
        bash::BashTool,
        fs::{GlobTool, GrepTool, ReadFileTool, WriteFileTool},
        subagent::SubagentTool,
        todo::{TodoReadTool, TodoWriteTool},
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let task_list = Arc::new(Mutex::new(TaskList::load(".cori_tasks.json")?));

    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    registry.register(TodoReadTool::new(Arc::clone(&task_list)));
    registry.register(TodoWriteTool::new(Arc::clone(&task_list)));
    registry.register(SubagentTool);
    // Session 07：新增文件系统工具
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(GlobTool);
    registry.register(GrepTool);

    let llm = ClaudeLlm::from_env(registry.all_schemas())?;
    let mut agent = AgentLoop::new(llm, registry);

    // 让 Agent 用文件系统工具探索项目结构
    let answer = agent
        .run(
            "请用 glob 和 read_file 工具完成以下分析：\
            \n1. 用 glob 找出 cori-core/src 下所有 .rs 文件\
            \n2. 用 read_file 读取 cori-core/src/lib.rs\
            \n3. 用 grep 在 cori-core/src 下搜索所有包含 'pub trait' 的行\
            \n4. 汇总：这个项目定义了哪些公开 trait？每个 trait 在哪个文件里？",
        )
        .await?;

    println!("\n{answer}");

    Ok(())
}

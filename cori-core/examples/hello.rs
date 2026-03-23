use std::sync::{Arc, Mutex};

use cori_core::{
    claude::ClaudeLlm,
    loop_::AgentLoop,
    planner::TaskList,
    tools::{
        ToolRegistry,
        bash::BashTool,
        subagent::SubagentTool,
        todo::{TodoReadTool, TodoWriteTool},
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 开启日志，能看到每次工具调用
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let task_list = Arc::new(Mutex::new(TaskList::load(".cori_tasks.json")?));

    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    registry.register(TodoReadTool::new(Arc::clone(&task_list)));
    registry.register(TodoWriteTool::new(Arc::clone(&task_list)));
    registry.register(SubagentTool);

    let llm = ClaudeLlm::from_env(registry.all_schemas())?;
    let mut agent = AgentLoop::new(llm, registry);

    // Prompt 设计要点：
    //   1. 明确要求用 spawn_subagent（否则 Claude 会直接用 bash 一次做完）
    //   2. 三个子任务彼此独立，适合并行委托
    //   3. 要求最后汇总，让父 Agent 有"整合结果"的角色
    let answer = agent
        .run(
            "请完成以下工作，要求：\
            - 先用 todo_write 列出三个子任务的计划 \
            - 然后对每个子任务分别调用 spawn_subagent 执行（每次只传一个独立子任务） \
            - 最后汇总三个子 Agent 的结果 \
            \n\
            三个子任务：\
            \n1. 统计这个 Rust 项目里有多少个 .rs 源文件（在 cori-core/src 下）\
            \n2. 找出代码中用了哪些外部 crate（查看 cori-core/Cargo.toml 的 [dependencies]）\
            \n3. 数一数项目里有多少个 #[cfg(test)] 测试模块",
        )
        .await?;

    println!("\n{answer}");

    println!("\n── 最终任务列表 ──");
    println!("{}", task_list.lock().unwrap().display());

    Ok(())
}

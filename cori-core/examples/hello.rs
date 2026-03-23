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
            "请严格按以下步骤完成工作：\
            \n\
            \n步骤 1：用 todo_write 创建三条任务，状态均为 pending \
            \n步骤 2：将任务 1 改为 in_progress，用 spawn_subagent 执行：统计 cori-core/src 下有多少个 .rs 文件 \
            \n步骤 3：将任务 1 改为 completed，将任务 2 改为 in_progress，用 spawn_subagent 执行：查看 cori-core/Cargo.toml 的 [dependencies] 列出所有依赖 \
            \n步骤 4：将任务 2 改为 completed，将任务 3 改为 in_progress，用 spawn_subagent 执行：统计项目中有多少个 #[cfg(test)] \
            \n步骤 5：将任务 3 改为 completed，汇总三个子 Agent 的结果",
        )
        .await?;

    println!("\n{answer}");

    println!("\n── 最终任务列表 ──");
    println!("{}", task_list.lock().unwrap().display());

    Ok(())
}

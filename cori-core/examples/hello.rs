use cori_core::{
    claude::ClaudeLlm,
    loop_::AgentLoop,
    tools::{ToolRegistry, bash::BashTool},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut registry = ToolRegistry::new();
    registry.register(BashTool);

    let llm = ClaudeLlm::from_env(registry.all_schemas())?;
    let mut agent = AgentLoop::new(llm, registry);

    let answer = agent.run("用 bash 工具执行 echo 'Cori is alive!'").await?;
    println!("{answer}");

    Ok(())
}

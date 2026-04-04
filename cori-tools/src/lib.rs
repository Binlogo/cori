//! Built-in tools for the Cori agent framework.
//!
//! This crate re-exports all tool implementations from `cori-core::tools`
//! and provides convenience functions for tool registration.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use cori_tools::register_defaults;
//! use cori_core::tools::ToolRegistry;
//!
//! let mut registry = ToolRegistry::new();
//! register_defaults(&mut registry).unwrap();
//! ```

pub use cori_core::tools::{
    bash::BashTool,
    edit::EditFileTool,
    fs::{GlobTool, GrepTool, ReadFileTool, WriteFileTool},
    subagent::SubagentTool,
    task::{TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool},
    Tool, ToolRegistry,
};

/// Register all built-in Cori tools into the given registry.
///
/// This sets up:
/// - BashTool — shell command execution
/// - ReadFileTool, WriteFileTool, EditFileTool — file I/O
/// - GlobTool, GrepTool — file search
/// - TaskCreateTool, TaskListTool, TaskGetTool, TaskUpdateTool — task management
///
/// The task graph is persisted to `.tasks/` in the current directory.
pub fn register_defaults(registry: &mut ToolRegistry) -> anyhow::Result<()> {
    use std::sync::{Arc, Mutex};

    use cori_core::planner::TaskGraph;

    let task_graph = Arc::new(Mutex::new(TaskGraph::load(".tasks")?));

    registry.register(BashTool);
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(EditFileTool);
    registry.register(GlobTool);
    registry.register(GrepTool);
    registry.register(TaskCreateTool::new(Arc::clone(&task_graph)));
    registry.register(TaskListTool::new(Arc::clone(&task_graph)));
    registry.register(TaskGetTool::new(Arc::clone(&task_graph)));
    registry.register(TaskUpdateTool::new(Arc::clone(&task_graph)));

    Ok(())
}

/// Register only file system tools (no bash, no task management).
pub fn register_fs_tools(registry: &mut ToolRegistry) {
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(EditFileTool);
    registry.register(GlobTool);
    registry.register(GrepTool);
}

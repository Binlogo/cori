use crate::planner::*;
use tempfile::TempDir;

fn temp_graph() -> (TaskGraph, TempDir) {
    let dir = TempDir::new().unwrap();
    let graph = TaskGraph::load(dir.path()).unwrap();
    (graph, dir)
}

// ── create ────────────────────────────────────────────────────────────────────

#[test]
fn test_create_assigns_ids() {
    let (graph, _dir) = temp_graph();
    let t1 = graph.create("Setup", "Init project", None).unwrap();
    let t2 = graph.create("Write code", "Implement", None).unwrap();
    assert_eq!(t1.id, "1");
    assert_eq!(t2.id, "2");
    assert_eq!(t1.status, TaskStatus::Pending);
}

#[test]
fn test_create_persists_to_disk() {
    let dir = TempDir::new().unwrap();
    {
        let graph = TaskGraph::load(dir.path()).unwrap();
        graph.create("Task A", "desc", None).unwrap();
    }
    // 重新加载
    let graph2 = TaskGraph::load(dir.path()).unwrap();
    let task = graph2.get("1").unwrap();
    assert_eq!(task.subject, "Task A");
}

// ── get ───────────────────────────────────────────────────────────────────────

#[test]
fn test_get_missing_task_errors() {
    let (graph, _dir) = temp_graph();
    assert!(graph.get("999").is_err());
}

// ── update ────────────────────────────────────────────────────────────────────

#[test]
fn test_update_status() {
    let (graph, _dir) = temp_graph();
    graph.create("Task", "desc", None).unwrap();
    let updated = graph
        .update("1", Some(TaskStatus::InProgress), None, None, None, None, None)
        .unwrap();
    assert_eq!(updated.status, TaskStatus::InProgress);
    // 持久化
    assert_eq!(graph.get("1").unwrap().status, TaskStatus::InProgress);
}

#[test]
fn test_update_deleted_hides_from_list() {
    let (graph, _dir) = temp_graph();
    graph.create("Task", "desc", None).unwrap();
    graph
        .update("1", Some(TaskStatus::Deleted), None, None, None, None, None)
        .unwrap();
    // display 不显示已删除任务
    let out = graph.display().unwrap();
    assert_eq!(out, "No tasks.");
}

// ── 双向链接维护 ──────────────────────────────────────────────────────────────

#[test]
fn test_add_blocked_by_maintains_bidirectional_link() {
    let (graph, _dir) = temp_graph();
    graph.create("Task 1", "", None).unwrap();
    graph.create("Task 2", "", None).unwrap();

    // task 2 depends on task 1
    graph
        .update("2", None, None, None, Some(vec!["1".into()]), None, None)
        .unwrap();

    let t1 = graph.get("1").unwrap();
    let t2 = graph.get("2").unwrap();

    assert!(t2.blocked_by.contains(&"1".to_string()), "task2.blocked_by should contain 1");
    assert!(t1.blocks.contains(&"2".to_string()), "task1.blocks should contain 2 (auto-maintained)");
}

#[test]
fn test_add_blocks_maintains_bidirectional_link() {
    let (graph, _dir) = temp_graph();
    graph.create("Task 1", "", None).unwrap();
    graph.create("Task 2", "", None).unwrap();

    // task 1 blocks task 2 (declared from the other direction)
    graph
        .update("1", None, None, None, None, Some(vec!["2".into()]), None)
        .unwrap();

    let t1 = graph.get("1").unwrap();
    let t2 = graph.get("2").unwrap();

    assert!(t1.blocks.contains(&"2".to_string()), "task1.blocks should contain 2");
    assert!(t2.blocked_by.contains(&"1".to_string()), "task2.blocked_by should contain 1 (auto-maintained)");
}

#[test]
fn test_no_duplicate_links() {
    let (graph, _dir) = temp_graph();
    graph.create("Task 1", "", None).unwrap();
    graph.create("Task 2", "", None).unwrap();

    // 重复添加同一依赖
    graph.update("2", None, None, None, Some(vec!["1".into()]), None, None).unwrap();
    graph.update("2", None, None, None, Some(vec!["1".into()]), None, None).unwrap();

    let t2 = graph.get("2").unwrap();
    assert_eq!(t2.blocked_by.iter().filter(|x| x.as_str() == "1").count(), 1);
}

// ── display 分区 ──────────────────────────────────────────────────────────────

#[test]
fn test_display_sections() {
    let (graph, _dir) = temp_graph();
    graph.create("Task 1", "", None).unwrap();
    graph.create("Task 2", "", None).unwrap();
    graph.create("Task 3", "", None).unwrap();

    // 1 完成，2 依赖 1（应变为 ready），3 依赖 2（仍 blocked）
    graph.update("1", Some(TaskStatus::Completed), None, None, None, None, None).unwrap();
    graph.update("2", None, None, None, Some(vec!["1".into()]), None, None).unwrap();
    graph.update("3", None, None, None, Some(vec!["2".into()]), None, None).unwrap();

    let out = graph.display().unwrap();
    assert!(out.contains("READY"), "task2 should be ready");
    assert!(out.contains("BLOCKED"), "task3 should be blocked");
    assert!(out.contains("COMPLETED"), "task1 should be completed");
    // BLOCKED 行不应把已完成的 #1 列为 open blocker
    let blocked_line = out.lines().find(|l| l.contains("blocked by")).unwrap_or("");
    assert!(!blocked_line.contains("#1"), "completed dep #1 should not appear in open blockers");
}

#[test]
fn test_display_open_blockers_only() {
    let (graph, _dir) = temp_graph();
    graph.create("Task 1", "", None).unwrap();
    graph.create("Task 2", "", None).unwrap();
    graph.create("Task 3", "", None).unwrap();

    // task 3 depends on task 1 (completed) and task 2 (pending)
    graph.update("1", Some(TaskStatus::Completed), None, None, None, None, None).unwrap();
    graph.update("3", None, None, None, Some(vec!["1".into(), "2".into()]), None, None).unwrap();

    let out = graph.display().unwrap();
    // task 3 is blocked by task 2 (open), NOT task 1 (completed)
    assert!(out.contains("#2"), "open blocker #2 should show");
    // The blocked line should not mention #1 as an open blocker
    let blocked_line = out.lines().find(|l| l.contains("blocked by")).unwrap_or("");
    assert!(!blocked_line.contains("#1"), "completed dep #1 should not appear in open blockers");
}

#[test]
fn test_empty_display() {
    let (graph, _dir) = temp_graph();
    assert_eq!(graph.display().unwrap(), "No tasks.");
}

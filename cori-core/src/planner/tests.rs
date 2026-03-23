use crate::planner::*;

use tempfile::NamedTempFile;

fn temp_list() -> (TaskList, NamedTempFile) {
    let f = NamedTempFile::new().unwrap();
    let list = TaskList::load(f.path()).unwrap();
    (list, f)
}

/// write 后能通过 display 看到任务
#[test]
fn test_write_and_display() {
    let (mut list, _f) = temp_list();
    list.write(vec![
        Task::new("1", "分析项目结构"),
        Task::new("2", "实现功能"),
    ])
    .unwrap();

    let output = list.display();
    assert!(output.contains("分析项目结构"));
    assert!(output.contains("实现功能"));
    assert!(output.contains("[ ]")); // 新任务是 pending
}

/// write 后从磁盘重新 load，数据一致
#[test]
fn test_persistence() {
    let f = NamedTempFile::new().unwrap();
    {
        let mut list = TaskList::load(f.path()).unwrap();
        list.write(vec![Task {
            id: "1".into(),
            description: "持久化测试".into(),
            state: TaskState::InProgress,
        }])
        .unwrap();
    }
    // 重新加载
    let list = TaskList::load(f.path()).unwrap();
    assert_eq!(list.tasks().len(), 1);
    assert_eq!(list.tasks()[0].state, TaskState::InProgress);
}

/// display 的状态符号正确
#[test]
fn test_display_symbols() {
    let (mut list, _f) = temp_list();
    list.write(vec![
        Task {
            id: "1".into(),
            description: "a".into(),
            state: TaskState::Pending,
        },
        Task {
            id: "2".into(),
            description: "b".into(),
            state: TaskState::InProgress,
        },
        Task {
            id: "3".into(),
            description: "c".into(),
            state: TaskState::Completed,
        },
    ])
    .unwrap();

    let output = list.display();
    assert!(output.contains("[ ]"));
    assert!(output.contains("[→]"));
    assert!(output.contains("[✓]"));
}

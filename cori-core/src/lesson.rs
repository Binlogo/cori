/// Lesson metadata — used by the web platform to render course navigation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Lesson {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

/// The full lesson catalog.
/// Add a new entry here when you start a new session.
pub fn catalog() -> Vec<Lesson> {
    vec![
        Lesson {
            id: "01-agent-loop",
            title: "Session 01 · The Agent Loop",
            description: "One loop & one tool — that's all an agent is.",
        },
        Lesson {
            id: "02-tool-dispatch",
            title: "Session 02 · Tool Dispatch",
            description: "Tools aren't magic — they're a dispatch map.",
        },
        Lesson {
            id: "03-real-api",
            title: "Session 03 · Real API Call",
            description: "Replace the mock — speak to Claude for real.",
        },
        Lesson {
            id: "04-context",
            title: "Session 04 · Context & Token Management",
            description: "Memory costs tokens — manage it or hit the wall.",
        },
    ]
}

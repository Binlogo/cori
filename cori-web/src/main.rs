use anyhow::Result;
use axum::{
    Router,
    extract::Path,
    response::{Html, IntoResponse},
    routing::get,
};
use pulldown_cmark::{Options, Parser, html};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("cori_web=debug".parse()?))
        .init();

    let app = Router::new()
        .route("/", get(index))
        .route("/lessons/:id", get(lesson));

    let addr = "127.0.0.1:3000";
    tracing::info!("Cori running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> impl IntoResponse {
    let lessons = cori_core::lesson::catalog();

    let items: String = lessons
        .iter()
        .map(|l| {
            format!(
                r#"<li><a href="/lessons/{}">{}</a><p>{}</p></li>"#,
                l.id, l.title, l.description
            )
        })
        .collect();

    Html(page(
        "Cori — How Claude Code Works",
        &format!(
            r#"
<header>
  <h1>Cori</h1>
  <p>An interactive guide to how Claude Code works — built in Rust, from scratch.</p>
</header>
<main>
  <h2>Sessions</h2>
  <ul class="lessons">{items}</ul>
</main>"#
        ),
    ))
}

async fn lesson(Path(id): Path<String>) -> impl IntoResponse {
    let lessons_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("lessons");

    let readme = lessons_dir.join(&id).join("README.md");

    let markdown = match std::fs::read_to_string(&readme) {
        Ok(s) => s,
        Err(_) => {
            return Html(page(
                "Not Found",
                "<p>Lesson not found. Check the <code>lessons/</code> directory.</p>",
            ))
        }
    };

    let mut html_output = String::new();
    let parser = Parser::new_ext(&markdown, Options::all());
    html::push_html(&mut html_output, parser);

    Html(page(&id, &format!("<main class=\"lesson\">{html_output}</main>")))
}

fn page(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} — Cori</title>
  <style>
    :root {{
      --bg: #0d1117; --fg: #e6edf3; --accent: #58a6ff;
      --muted: #8b949e; --border: #30363d; --code-bg: #161b22;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{ background: var(--bg); color: var(--fg); font: 16px/1.7 'Segoe UI', system-ui, sans-serif; padding: 2rem; }}
    a {{ color: var(--accent); text-decoration: none; }}
    a:hover {{ text-decoration: underline; }}
    header {{ border-bottom: 1px solid var(--border); padding-bottom: 1.5rem; margin-bottom: 2rem; }}
    header h1 {{ font-size: 2rem; color: var(--accent); }}
    header p {{ color: var(--muted); margin-top: 0.4rem; }}
    h2 {{ font-size: 1.3rem; margin: 1.5rem 0 0.8rem; }}
    ul.lessons {{ list-style: none; }}
    ul.lessons li {{ border: 1px solid var(--border); border-radius: 8px; padding: 1rem 1.2rem; margin-bottom: 0.8rem; }}
    ul.lessons li a {{ font-size: 1.05rem; font-weight: 600; }}
    ul.lessons li p {{ color: var(--muted); font-size: 0.9rem; margin-top: 0.3rem; }}
    .lesson {{ max-width: 800px; }}
    .lesson h1 {{ font-size: 1.8rem; margin-bottom: 1rem; }}
    .lesson h2 {{ border-top: 1px solid var(--border); padding-top: 1rem; }}
    .lesson h3 {{ margin: 1.2rem 0 0.5rem; color: var(--accent); }}
    .lesson p {{ margin: 0.6rem 0; }}
    .lesson ul, .lesson ol {{ padding-left: 1.5rem; margin: 0.5rem 0; }}
    .lesson pre {{ background: var(--code-bg); border: 1px solid var(--border); border-radius: 6px; padding: 1rem; overflow-x: auto; margin: 0.8rem 0; }}
    .lesson code {{ font-family: 'Fira Code', 'Cascadia Code', monospace; font-size: 0.88rem; }}
    .lesson p code {{ background: var(--code-bg); padding: 0.1em 0.4em; border-radius: 4px; }}
    blockquote {{ border-left: 3px solid var(--accent); padding-left: 1rem; color: var(--muted); margin: 1rem 0; }}
  </style>
</head>
<body>
{body}
</body>
</html>"#
    )
}

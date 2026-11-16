//! `paperweight-mcp`: Paperweight's MCP server over stdio.
//!
//! Stdout carries protocol messages only. Everything else (logs, errors)
//! goes to stderr.
#![deny(clippy::print_stdout)]

mod server;

use std::path::PathBuf;

use anyhow::Context;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

const HELP: &str = "paperweight-mcp — Paperweight's MCP server (stdio)

Usage: paperweight-mcp [--demo | --seed-demo] [--db PATH]

Started by your AI client; it speaks MCP on stdin/stdout.

Options:
  --db PATH     Use this database (default: $PAPERWEIGHT_DB, or the OS data directory)
  --demo        Use the separate demo database
  --seed-demo   Reset the demo database to a realistic week, then serve it
  --version     Print the version to stderr
  --help        Print this help to stderr
";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_env("PAPERWEIGHT_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let mut db: Option<PathBuf> = None;
    let mut demo = false;
    let mut seed = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--db" => db = Some(args.next().context("--db needs a path")?.into()),
            "--demo" => demo = true,
            "--seed-demo" => {
                demo = true;
                seed = true;
            }
            "--version" | "-V" => {
                eprintln!("paperweight-mcp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                eprint!("{HELP}");
                return Ok(());
            }
            other => {
                eprint!("Unknown argument: {other}\n\n{HELP}");
                std::process::exit(2);
            }
        }
    }
    let path = match (db, demo) {
        (Some(p), _) => p,
        (None, true) => paperweight_core::db::demo_path(),
        (None, false) => paperweight_core::db::default_path(),
    };
    if seed {
        paperweight_core::demo::reset_demo(&path, chrono::Utc::now())
            .with_context(|| format!("seeding demo database at {}", path.display()))?;
        tracing::info!(path = %path.display(), "demo database seeded");
    } else {
        // Run migrations up front so a broken database fails loudly at start.
        paperweight_core::Store::open(&path)
            .with_context(|| format!("opening database at {}", path.display()))?;
    }
    tracing::info!(db = %path.display(), version = env!("CARGO_PKG_VERSION"), "paperweight-mcp starting");

    let service = server::Paperweight::new(path)
        .serve(rmcp::transport::stdio())
        .await
        .inspect_err(|e| tracing::error!("failed to start MCP service: {e}"))?;
    let reason = service.waiting().await?;
    tracing::info!(?reason, "paperweight-mcp stopped");
    Ok(())
}

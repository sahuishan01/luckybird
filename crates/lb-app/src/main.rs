mod app;
mod overlay;
mod paths;
mod theme;
mod commands;
mod compile;
mod event_loop;
mod installer;

use std::path::PathBuf;

use anyhow::Context;
use lb_config::Config;
use lb_lspmgr::LspManager;
use lb_plugin::PluginRuntime;
use lb_syntax::SyntaxManager;
use lb_tui::Tui;

// ─── CLI ─────────────────────────────────────────────────────────────────────

struct CliArgs {
    files: Vec<PathBuf>,
    headless: bool,
}

fn parse_args() -> anyhow::Result<CliArgs> {
    let mut files = Vec::new();
    let mut headless = false;
    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        match arg {
            lexopt::Arg::Short('h') | lexopt::Arg::Long("help") => {
                eprintln!("Usage: luckybird [OPTIONS] [FILE]...");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  -h, --help        Print help");
                eprintln!("  -v, --version     Print version");
                eprintln!("      --headless    Start without opening TUI (for testing)");
                std::process::exit(0);
            }
            lexopt::Arg::Short('v') | lexopt::Arg::Long("version") => {
                eprintln!("luckybird {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            lexopt::Arg::Long("headless") => {
                headless = true;
            }
            lexopt::Arg::Value(val) => {
                files.push(PathBuf::from(val));
            }
            _ => return Err(arg.unexpected().into()),
        }
    }

    Ok(CliArgs { files, headless })
}

// ─── Data directories ─────────────────────────────────────────────────────────

fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("luckybird")
}

fn config_grammar_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("luckybird")
        .join("grammars")
}

fn config_query_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("luckybird")
        .join("queries")
}

fn lsp_cache_dir() -> PathBuf {
    data_dir().join("lsp")
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Initialise logging to a file (never stderr — that would corrupt the TUI).
    let log_path = data_dir().join("luckybird.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    init_logging(&log_path);

    let args = parse_args()?;

    if args.headless {
        // Headless mode: open files and exit immediately (used in tests).
        tracing::info!("headless mode, exiting");
        return Ok(());
    }

    // Load config.
    let config = Config::load_default().unwrap_or_default();

    // Build subsystems.
    let syntax = SyntaxManager::new(config_grammar_dir(), config_query_dir());
    let workspace_root = args
        .files
        .first()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let lsp = LspManager::new(lsp_cache_dir(), workspace_root);
    let plugins = PluginRuntime::new();
    if !config.plugins.is_empty() {
        tracing::warn!("[[plugin]] entries are ignored: plugins are compiled in as Rust crates");
    }

    // Enter TUI.
    let tui = Tui::enter().context("entering TUI")?;

    let (job_tx, mut job_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let installer = installer::Installer::new(job_tx, config_grammar_dir(), config_query_dir());
    let mut app = app::App::new(config, tui, syntax, lsp, plugins, installer);

    // Open files from CLI.
    for file in &args.files {
        if let Err(e) = app.open_file(file.clone()) {
            app.status_msg = Some(format!("Cannot open {}: {e}", file.display()));
        }
    }

    // Start event thread and run loop.
    let mut event_rx = event_loop::spawn_event_thread();
    event_loop::run(&mut app, &mut event_rx, &mut job_rx).await;

    // Restore terminal before exit.
    app.tui.leave().context("leaving TUI")?;

    Ok(())
}

// ─── Logging ──────────────────────────────────────────────────────────────────

fn init_logging(log_path: &std::path::Path) {
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;

    // Write to file; ignore errors (logging failure must not crash the editor).
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer()
                .with_writer(std::sync::Mutex::new(file))
                .with_ansi(false),
        );
        let _ = tracing::subscriber::set_global_default(subscriber);
    }
}

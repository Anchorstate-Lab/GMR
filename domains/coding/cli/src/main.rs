mod cli;
mod error;
mod render;
mod rules;
mod verbs;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use gmr::Runtime;
use gmr_provider_git::Git;
use gmr_transport_shell::Shell;

use cli::{Cli, Command};
use error::CliError;

/// Probe artifact store: content-addressed and colocated with the journal.
pub(crate) fn probes_dir(root: &std::path::Path) -> PathBuf {
    root.join(".anchor").join("probes")
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("anchor: {e}");
            ExitCode::from(2)
        }
    }
}

async fn run(cli: Cli) -> Result<i32, CliError> {
    let root = PathBuf::from(&cli.repo)
        .canonicalize()
        .map_err(|e| CliError(format!("cannot find repository `{}`: {e}", cli.repo)))?;

    // Publishing a probe does not touch the journal; it happens before any log exists.
    if let Command::Publish {
        from,
        entrypoint,
        args,
        env,
    } = cli.command
    {
        return verbs::publish::run(&root, from, entrypoint, args, env, cli.json);
    }

    let dir = root.join(".anchor");
    std::fs::create_dir_all(&dir).map_err(|e| CliError(format!("cannot create .anchor: {e}")))?;
    let store = gmr::sqlite::open(dir.join("memory.db")).await?;

    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(&root, probes_dir(&root))))
        .provider(Arc::new(Git::new(&root)))
        .queue(Arc::new(store.queue()))
        .journal(Arc::new(store.journal()))
        .bindings(Arc::new(store.bindings()))
        .build();

    let json = cli.json;
    match cli.command {
        Command::Sync { file, dry_run } => verbs::sync::run(&rt, &root, file, dry_run, json).await,
        Command::Publish { .. } => unreachable!("publish was handled above"),
        Command::Open(args) => verbs::open::run(&rt, args, json).await,
        Command::Observe { key } => verbs::observe::run(&rt, key, json).await,
        Command::Read { key, moved } => verbs::read::run(&rt, key, moved, json).await,
        Command::Reprobe {
            key,
            artifact,
            params,
            why,
        } => verbs::reprobe::run(&rt, key, artifact, params, why, json).await,
        Command::Retransition { key, rules, why } => {
            verbs::retransition::run(&rt, key, rules, why, json).await
        }
        Command::Reterminal { key, terminal, why } => {
            verbs::reterminal::run(&rt, key, terminal, why, json).await
        }
        Command::Restate { key, state, why } => {
            verbs::restate::run(&rt, key, state, why, json).await
        }
        Command::Bind {
            path,
            anchors,
            detach,
        } => verbs::bind::run(&rt, &root, path, anchors, detach, json).await,
        Command::Close { key, why } => verbs::close::run(&rt, key, why).await,
        Command::Edges { since, status } => verbs::edges::run(&rt, since, status, json).await,
        Command::Health { key } => verbs::edges::health(&rt, key, json).await,
        Command::Pass => verbs::pass::run(&rt, json).await,
        Command::Doctor => verbs::doctor::run(&rt, json).await,
    }
}

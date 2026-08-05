mod cli;
mod error;
mod memories;
mod probes;
mod render;
mod rules;
mod shapes;
mod skill;
mod verbs;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use gmr::Runtime;
use gmr_provider::claude_code::ClaudeMemory;
use gmr_provider::git::Git;
use gmr_transport::inproc::InProcess;
use gmr_transport::script::Script;
use gmr_transport::shell::Shell;

use cli::{Cli, Command};
use error::CliError;

/// Probe artifact store: content-addressed and colocated with the journal.
pub(crate) fn probes_dir(root: &std::path::Path) -> PathBuf {
    probes::store_dir(root)
}

/// The journal moved into state/. Never move it silently: starting on an empty
/// database would erase history that nothing can rebuild.
fn stale_journal_guard(root: &std::path::Path, state: &std::path::Path) -> Result<(), CliError> {
    let old = probes::anchor_dir(root).join("memory.db");
    if !old.is_file() || state.join("memory.db").is_file() {
        return Ok(());
    }
    Err(CliError(format!(
        "the journal now lives in {}, but a database is still at {}.\n\
         Move it yourself, siblings included — the -wal file holds entries that are not in the .db yet:\n\
         \n    mkdir -p {0} && mv {1}* {0}/\n",
        state.display(),
        old.display()
    )))
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("gmr: {e}");
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
        name,
        entrypoint,
        args,
        env,
    } = cli.command
    {
        return verbs::publish::run(&root, from, name, entrypoint, args, env, cli.json);
    }

    // Building probes does not touch the journal either.
    if let Command::Probes(cmd) = cli.command {
        return match cmd {
            cli::ProbesCmd::Build => verbs::probes::build(&root, cli.json),
            cli::ProbesCmd::List { verbose } => verbs::probes::list(&root, verbose, cli.json),
            cli::ProbesCmd::Bundle { out } => {
                verbs::probes::bundle(&root, std::path::Path::new(&out), cli.json)
            }
        };
    }

    if let Command::Init { global } = cli.command {
        return verbs::init::run(&root, cli.json, global);
    }

    let state = probes::state_dir(&root);
    stale_journal_guard(&root, &state)?;
    std::fs::create_dir_all(&state)
        .map_err(|e| CliError(format!("cannot create {state:?}: {e}")))?;
    let store = gmr::sqlite::open(state.join("memory.db")).await?;

    // Export and import work on the raw store, not the Runtime — a version
    // gap is exactly the case where building a Runtime is the wrong move.
    if let Command::Export { out } = cli.command {
        return verbs::export::run(&store, out, cli.json).await;
    }
    if let Command::Import { file } = cli.command {
        return verbs::import::run(&store, file, cli.json).await;
    }

    // Three transports, one router: the extractors are linked in, a user's own
    // script is a file in their repo, and an artifact is still exec'd.
    let catalog = probes::Catalog::load(&root)?;
    let mut builder = Runtime::builder()
        .transport(Arc::new(InProcess::new(&root, coding_extract::registry())))
        .transport(Arc::new(Script::new(&root, catalog.script_paths())))
        .transport(Arc::new(Shell::new(&root, probes_dir(&root))))
        .provider(Arc::new(Git::new(&root)))
        .queue(Arc::new(store.queue()))
        .settings(Arc::new(store.queue()))
        .journal(Arc::new(store.journal()))
        .bindings(Arc::new(store.bindings()))
        .sealer(Arc::new(store.bindings()))
        .links(Arc::new(store.links()));
    // Read-only and additive: absence of Claude Code's own memory directory
    // is normal outside a Claude Code session, not a reason to refuse to run.
    match ClaudeMemory::new(&root) {
        Ok(p) => builder = builder.provider(Arc::new(p)),
        Err(e) => eprintln!("gmr: claude-code memory provider unavailable: {e}"),
    }
    let rt = builder.build();

    let json = cli.json;
    match cli.command {
        Command::Sync { file, dry_run } => verbs::sync::run(&rt, &root, file, dry_run, json).await,
        Command::Publish { .. } => unreachable!("publish was handled above"),
        Command::Probes(_) => unreachable!("probes was handled above"),
        Command::Init { .. } => unreachable!("init was handled above"),
        Command::Open(args) => verbs::open::run(&rt, &root, args, json).await,
        Command::Observe { key } => verbs::observe::run(&rt, key, json).await,
        Command::Read { key } => verbs::read::run(&rt, key, json).await,
        Command::Reprobe {
            key,
            probe,
            params,
            why,
        } => verbs::reprobe::run(&rt, &root, key, probe, params, why, json).await,
        Command::Retransition { key, rules, why } => {
            verbs::retransition::run(&rt, key, rules, why, json).await
        }
        Command::Reterminal { key, terminal, why } => {
            verbs::reterminal::run(&rt, key, terminal, why, json).await
        }
        Command::Rebase { keys, all, why } => verbs::rebase::run(&rt, keys, all, why, json).await,
        Command::Restate { key, state, why } => {
            verbs::restate::run(&rt, key, state, why, json).await
        }
        Command::Bind {
            path,
            anchors,
            detach,
            provider,
        } => verbs::bind::run(&rt, &root, path, anchors, detach, provider, json).await,
        Command::Reaffirm { path } => verbs::reaffirm::run(&rt, &root, path, json).await,
        Command::Cobound { path } => verbs::cobound::run(&rt, path, json).await,
        Command::Link { from, to, kind } => verbs::link::run(&rt, from, to, kind, json).await,
        Command::Close { key, why } => verbs::close::run(&rt, key, why).await,
        Command::Edges { since, status } => verbs::edges::run(&rt, since, status, json).await,
        Command::Health { key } => verbs::health::run(&rt, key, json).await,
        Command::Requeue { key } => verbs::requeue::run(&rt, key, json).await,
        Command::Pass => verbs::pass::run(&rt, json).await,
        Command::Doctor => verbs::doctor::run(&rt, &root, json).await,
        Command::Export { .. } => unreachable!("export was handled above"),
        Command::Import { .. } => unreachable!("import was handled above"),
    }
}

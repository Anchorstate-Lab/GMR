mod cli;
mod contract;
mod coord;
mod delivery;
mod error;
mod memories;
mod notes;
mod probes;
mod prose;
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
use gmr_provider::mem0::{Mem0, Scope};
use gmr_transport::inproc::InProcess;
use gmr_transport::script::Script;
use gmr_transport::shell::Shell;

use cli::{Cli, Command};
use error::CliError;

pub(crate) fn probes_dir(root: &std::path::Path) -> PathBuf {
    probes::store_dir(root)
}

fn mem0() -> Option<Result<Mem0, gmr::ContentError>> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let scope = Scope {
        user_id: env("MEM0_USER_ID"),
        agent_id: env("MEM0_AGENT_ID"),
        app_id: env("MEM0_APP_ID"),
    };
    match (env("MEM0_BASE_URL"), env("MEM0_API_KEY")) {
        (Some(base), key) => Some(Mem0::self_hosted(base, key, scope)),
        (None, Some(key)) => Some(Mem0::platform(key, scope)),
        (None, None) => None,
    }
}

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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("gmr: cannot start the runtime: {e}");
            return ExitCode::from(2);
        }
    };
    let outcome = runtime.block_on(run(cli));
    runtime.shutdown_background();
    match outcome {
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

    let outcome = served(cli, root, state, &store).await;
    store.close().await;
    outcome
}

async fn served(
    cli: Cli,
    root: PathBuf,
    state: PathBuf,
    store: &gmr::sqlite::SqliteStore,
) -> Result<i32, CliError> {
    if let Command::Export { out } = cli.command {
        return verbs::export::run(store, out, cli.json).await;
    }
    if let Command::Import { file } = cli.command {
        return verbs::import::run(store, file, cli.json).await;
    }

    let catalog = probes::Catalog::load(&root)?;
    let linked = coding_extract::registry(&state);
    if let Some(fault) = &linked.cache_fault {
        eprintln!("gmr: {fault}");
    }
    let mut builder = Runtime::builder()
        .policy(gmr::Policy {
            probe_budget_ms: cli.probe_budget_ms,
            ..Default::default()
        })
        .transport(Arc::new(InProcess::new(&root, linked.probes)))
        .transport(Arc::new(Script::new(&root, catalog.script_paths())))
        .transport(Arc::new(Shell::new(&root, probes_dir(&root))))
        .provider(Arc::new(Git::new(&root)))
        .queue(Arc::new(store.queue()))
        .settings(Arc::new(store.queue()))
        .journal(Arc::new(store.journal()))
        .bindings(Arc::new(store.bindings()))
        .sealer(Arc::new(store.bindings()))
        .links(Arc::new(store.links()));
    match ClaudeMemory::new(&root) {
        Ok(p) => builder = builder.provider(Arc::new(p)),
        Err(e) => {
            eprintln!("gmr: claude-code memory provider unavailable: {e}");
            builder = builder.provider_warning("claude-code", e.to_string());
        }
    }
    if let Some(built) = mem0() {
        match built {
            Ok(p) => builder = builder.provider(Arc::new(p)),
            Err(e) => {
                eprintln!("gmr: mem0 provider unavailable: {e}");
                builder = builder.provider_warning("mem0", e.to_string());
            }
        }
    }
    let rt = builder.build();

    let json = cli.json;
    match cli.command {
        Command::Sync { file, dry_run } => verbs::sync::run(&rt, &root, file, dry_run, json).await,
        Command::Anchor { coordinate, memory } => {
            verbs::anchor::run(&rt, &root, coordinate, memory, json).await
        }
        Command::Status { key } => verbs::status::run(&rt, &root, key, json).await,
        Command::Check { key } => verbs::check::run(&rt, &root, key, json).await,
        Command::Atlas { out } => verbs::atlas::run(&rt, &root, out, json).await,
        Command::Publish { .. } => unreachable!("publish was handled above"),
        Command::Probes(_) => unreachable!("probes was handled above"),
        Command::Init { .. } => unreachable!("init was handled above"),
        Command::Open(args) => verbs::open::run(&rt, &root, args, json).await,
        Command::Observe { key } => verbs::observe::run(&rt, &root, key, json).await,
        Command::Accept {
            key,
            baseline,
            criteria,
            all,
            why,
        } => {
            let asked = match (baseline, criteria) {
                (true, _) => Some(verbs::accept::What::Baseline),
                (_, true) => Some(verbs::accept::What::Criteria),
                _ => None,
            };
            verbs::accept::run(&rt, &root, key, why, asked, all, json).await
        }
        Command::Read { key } => verbs::read::run(&rt, &root, key, json).await,
        Command::Revise(args) => verbs::revise::run(&rt, &root, args, json).await,
        Command::Rebase { keys, all, why } => verbs::rebase::run(&rt, keys, all, why, json).await,
        Command::Bind {
            path,
            anchors,
            detach,
            provider,
        } => verbs::bind::run(&rt, path, anchors, detach, provider, json).await,
        Command::Reaffirm { path, provider } => {
            verbs::reaffirm::run(&rt, path, provider, json).await
        }
        Command::Cobound { path, provider } => verbs::cobound::run(&rt, path, provider, json).await,
        Command::Link {
            from,
            to,
            kind,
            from_provider,
            to_provider,
        } => verbs::link::run(&rt, from, to, kind, from_provider, to_provider, json).await,
        Command::Close { key, why } => verbs::close::run(&rt, key, why).await,
        Command::Edges { since, status } => verbs::edges::run(&rt, since, status, json).await,
        Command::Health { key } => verbs::health::run(&rt, key, json).await,
        Command::Requeue { key } => verbs::requeue::run(&rt, key, json).await,
        Command::Pass => verbs::pass::run(&rt, &root, json).await,
        Command::Doctor => {
            verbs::doctor::run(&rt, &root, linked.cache_fault.as_deref(), json).await
        }
        Command::Export { .. } => unreachable!("export was handled above"),
        Command::Import { .. } => unreachable!("import was handled above"),
    }
}

pub mod cli;
pub mod contract;
pub mod coord;
pub mod delivery;
pub mod error;
pub mod memories;
pub mod notes;
pub mod probes;
pub mod prose;
pub mod providers;
pub mod render;
pub mod rules;
pub mod settings;
pub mod shapes;
pub mod skill;
pub mod stores;
pub mod verbs;

use std::path::PathBuf;
use std::sync::Arc;

use gmr::Runtime;
use gmr_transport::inproc::InProcess;
use gmr_transport::script::Script;
use gmr_transport::shell::Shell;

use cli::{Cli, Command};
use error::CliError;

pub fn probes_dir(root: &std::path::Path) -> PathBuf {
    probes::store_dir(root)
}

pub fn stale_journal_guard(
    root: &std::path::Path,
    state: &std::path::Path,
) -> Result<(), CliError> {
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

pub async fn run(cli: Cli) -> Result<i32, CliError> {
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

pub async fn served(
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
    let declared = Arc::new(probes::Declared::at(&root)?);
    let linked = coding_extract::registry(&root, &state).await;
    if let Some(fault) = &linked.cache_fault {
        eprintln!("gmr: {fault}");
    }
    let mut builder = Runtime::builder()
        .policy(gmr::Policy {
            probe_budget_ms: cli.probe_budget_ms,
            content_call_ms: cli.content_call_ms,
            content_total_ms: cli.content_total_ms,
            ..Default::default()
        })
        .transport(Arc::new(InProcess::new(&root, linked.probes)))
        .transport(Arc::new(Script::new(&root, catalog.script_paths())))
        .transport(Arc::new(Shell::new(&root, probes_dir(&root))))
        .transport(Arc::new(
            gmr_transport::http::Http::new(Arc::clone(&declared))
                .map_err(|e| CliError(format!("cannot build the http transport: {e}")))?,
        ))
        .transport(Arc::new(gmr_transport::file::Files::new(
            &root,
            Arc::clone(&declared),
        )))
        .transport(Arc::new(gmr_transport::sql::Sql::new(Arc::clone(
            &declared,
        ))))
        .queue(Arc::new(store.queue()))
        .settings(Arc::new(store.settings()))
        .sightings(Arc::new(store.sightings()))
        .journal(Arc::new(store.journal()))
        .bindings(Arc::new(store.bindings()))
        .sealer(Arc::new(store.sealer()))
        .links(Arc::new(store.links()));
    let stores = stores::assembled(&root)?;
    for store in &stores.built {
        builder = builder.provider(store.content());
    }
    for warning in &stores.warnings {
        eprintln!(
            "gmr: {} memory provider unavailable: {}",
            warning.provider, warning.message
        );
        builder = builder.provider_warning(&warning.provider, &warning.message);
    }
    let rt = builder.build();
    let names = &stores.names;

    let json = cli.json;
    match cli.command {
        Command::Sync { file, dry_run } => {
            verbs::sync::run(&rt, &root, names, file, dry_run, json).await
        }
        Command::Anchor {
            coordinate,
            named,
            memory,
            record,
        } => {
            let asked = verbs::anchor::Asked {
                coordinate,
                named,
                memory,
                record,
            };
            verbs::anchor::run(&rt, &root, &stores, asked, json).await
        }
        Command::Memories { provider } => verbs::memories::run(&rt, &stores, provider, json).await,
        Command::Status { key } => verbs::status::run(&rt, &root, names, key, json).await,
        Command::Check { key } => verbs::check::run(&rt, &root, names, key, json).await,
        Command::Said {
            text,
            on,
            saw,
            depends,
            id,
        } => {
            verbs::said::run(
                &rt,
                verbs::said::Said {
                    id,
                    text,
                    on,
                    saw,
                    depends,
                },
                json,
            )
            .await
        }
        Command::Standing { id, retire } => verbs::standing::run(&rt, id, retire, json).await,
        Command::Atlas { out } => verbs::atlas::run(&rt, &root, names, out, json).await,
        Command::Publish { .. } => unreachable!("publish was handled above"),
        Command::Probes(_) => unreachable!("probes was handled above"),
        Command::Init { .. } => unreachable!("init was handled above"),
        Command::Open(args) => verbs::open::run(&rt, &root, args, json).await,
        Command::Observe { key } => verbs::observe::run(&rt, &root, names, key, json).await,
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
        Command::Read {
            key,
            fresher_than_secs,
        } => verbs::read::run(&rt, names, key, fresher_than_secs, json).await,
        Command::Revise(args) => verbs::revise::run(&rt, &root, args, json).await,
        Command::Rebase { keys, all, why } => {
            verbs::rebase::run(&rt, &root, names, keys, all, why, json).await
        }
        Command::Bind {
            path,
            anchors,
            detach,
            provider,
        } => {
            let reference = stores.locate(&path, provider.as_deref())?;
            verbs::bind::run(&rt, names, reference, anchors, detach, json).await
        }
        Command::Attest {
            path,
            anchors,
            provider,
        } => {
            let reference = stores.locate(&path, provider.as_deref())?;
            verbs::bind::attest(&rt, names, reference, anchors, json).await
        }
        Command::Reaffirm { path, provider } => {
            let reference = stores.locate(&path, provider.as_deref())?;
            verbs::reaffirm::run(&rt, names, reference, json).await
        }
        Command::Cobound { path, provider } => {
            let reference = stores.locate(&path, provider.as_deref())?;
            verbs::cobound::run(&rt, names, reference, json).await
        }
        Command::Link {
            from,
            to,
            kind,
            detach,
            from_provider,
            to_provider,
        } => {
            let from = stores.locate(&from, from_provider.as_deref())?;
            let to = stores.locate(&to, to_provider.as_deref())?;
            verbs::link::run(&rt, from, to, kind, detach, json).await
        }
        Command::Close { key, why } => verbs::close::run(&rt, key, why).await,
        Command::Edges { since, status } => {
            verbs::edges::run(&rt, names, since, status, json).await
        }
        Command::Health { key } => verbs::health::run(&rt, key, json).await,
        Command::Requeue { key } => verbs::requeue::run(&rt, key, json).await,
        Command::Pass => verbs::pass::run(&rt, &root, names, json).await,
        Command::Doctor => {
            let broken = gmr::Chained::chain_break(&store.journal()).await?;
            verbs::doctor::run(
                &rt,
                &root,
                names,
                linked.cache_fault.as_deref(),
                broken,
                json,
            )
            .await
        }
        Command::Export { .. } => unreachable!("export was handled above"),
        Command::Import { .. } => unreachable!("import was handled above"),
    }
}

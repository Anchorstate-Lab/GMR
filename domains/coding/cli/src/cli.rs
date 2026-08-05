use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gmr",
    version,
    about = "Attach judgment to recomputable observations",
    long_about = "You write the memories; anchors are mechanical observations.\n\
                  Probes say what to inspect, transition tables say what counts as change,\n\
                  and this tool ships no built-in criteria.\n\
                  \n\
                  Start with `init`, write a note naming the coordinate it is about,\n\
                  `sync` to open it, then `observe` to ask whether it still holds.",
    after_help = "The path, in five steps:\n\
                  \n  \
                  gmr init                    create .anchor/, install probes\n  \
                  <write a note>              ---\\nabout: src/x.ts#name\\n---\n  \
                  gmr sync                    open what the notes declare, bind them\n  \
                  gmr observe                 has the world moved?\n  \
                  gmr pass --json             what moved, and the notes bound to it\n\
                  \n\
                  Commands are listed declare, observe, revise, relate. Every revise\n\
                  verb takes --why and seals it: changing criteria is a judgment."
)]
pub struct Cli {
    #[arg(long, default_value = ".", global = true)]
    pub repo: String,

    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create .anchor/, register bundled probes, and report what is readable.
    #[command(display_order = 0)]
    Init {
        /// Write the bundled skill doc to ~/.claude/skills/gmr/ instead of
        /// this project's .claude/skills/gmr/.
        #[arg(long)]
        global: bool,
    },

    /// Build every declared probe recipe and install it for this machine.
    #[command(subcommand)]
    #[command(display_order = 1)]
    Probes(ProbesCmd),

    /// Open what the declarations and notes ask for, and align bindings.
    #[command(display_order = 2)]
    Sync {
        #[arg(default_value = crate::verbs::sync::DEFAULT_FILE)]
        file: String,
        #[arg(long)]
        dry_run: bool,
    },

    /// Open one anchor directly, naming its probe and rules by hand.
    #[command(display_order = 3)]
    Open(OpenArgs),

    /// Ask every anchor whether the world still matches. Exit 1 if any moved.
    #[command(display_order = 10)]
    Observe { key: Option<String> },

    /// Each anchor's current state.
    #[command(display_order = 12)]
    Read { key: Option<String> },

    /// Install a directory as a named probe artifact.
    #[command(display_order = 34)]
    Publish {
        from: String,
        /// The name anchors will write down.
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "probe")]
        entrypoint: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        /// Environment required by the probe (K=V). It enters the version and your responsibility.
        #[arg(long = "env")]
        env: Vec<String>,
    },

    /// Look somewhere else. Needs --why, and the reason is sealed.
    #[command(display_order = 20)]
    Reprobe {
        key: String,
        /// The probe to look with, by name.
        #[arg(long)]
        probe: String,
        #[arg(long, default_value = "{}")]
        params: String,
        #[arg(long)]
        why: String,
    },

    /// Change what counts as a change. Needs --why, and the reason is sealed.
    #[command(display_order = 21)]
    Retransition {
        key: String,
        #[arg(long = "rule", value_name = "GUARD => NEW_STATE", required = true)]
        rules: Vec<String>,
        #[arg(long)]
        why: String,
    },

    /// Change what is irreversible. Needs --why, and the reason is sealed.
    #[command(display_order = 22)]
    Reterminal {
        key: String,
        #[arg(long = "terminal", value_delimiter = ',', required = true)]
        terminal: Vec<String>,
        #[arg(long)]
        why: String,
    },

    /// Recapture against the instrument this build has. Needs --why, and the
    /// reason is sealed.
    #[command(display_order = 25)]
    Rebase {
        keys: Vec<String>,
        /// Every anchor standing on a reading a different instrument took.
        #[arg(long)]
        all: bool,
        #[arg(long)]
        why: String,
    },

    /// Move the state directly. Needs --why, and the reason is sealed.
    #[command(display_order = 23)]
    Restate {
        key: String,
        #[arg(long)]
        state: String,
        #[arg(long)]
        why: String,
    },

    /// Say by hand which anchors a note is about; notes usually say it themselves.
    #[command(display_order = 30)]
    Bind {
        path: String,
        #[arg(long, value_delimiter = ',', conflicts_with = "detach")]
        anchors: Vec<String>,
        #[arg(long)]
        detach: bool,
        /// Which registered ContentProvider `path` is resolved through.
        /// What's actually available depends on how this binary was built.
        #[arg(long, default_value = "git")]
        provider: String,
    },

    /// Re-stamp a binding's content version without changing which anchors it's about.
    #[command(display_order = 31)]
    Reaffirm { path: String },

    /// Other references bound to any anchor `path` is also bound to.
    #[command(display_order = 32)]
    Cobound { path: String },

    /// Record that `from` relates to `to`. Independent of anchoring — linking
    /// two references says nothing about which anchors either is bound to.
    #[command(display_order = 33)]
    Link {
        from: String,
        to: String,
        #[arg(long)]
        kind: String,
    },

    /// Retire an anchor. Closure is irreversible.
    #[command(display_order = 24)]
    Close {
        key: String,
        #[arg(long)]
        why: String,
    },

    /// Transitions, terminals and stalls since a point in the journal.
    #[command(display_order = 13)]
    Edges {
        #[arg(long, default_value = "0")]
        since: u64,
        #[arg(long)]
        status: Option<String>,
    },

    /// Per-anchor liveness: last seen, attempts, backoff.
    #[command(display_order = 14)]
    Health { key: Option<String> },

    /// Force `key` due now, clearing any backoff or parked state. Unlike
    /// `sync`, which only repairs a missing queue row, this always resets it.
    #[command(display_order = 16)]
    Requeue { key: String },

    /// Observe only what is due, and hand back the notes bound to whatever moved.
    #[command(display_order = 11)]
    Pass,

    /// Anchors that were never seen, or that carry no note.
    #[command(display_order = 15)]
    Doctor,

    /// Snapshot the journal, bindings, links and sealed rationale as JSONL,
    /// independent of this database's schema version. Run this with the old
    /// binary before upgrading — `import` on the new one replays it.
    #[command(display_order = 40)]
    Export {
        /// Where to write the JSONL. Defaults to stdout, so it can be piped.
        #[arg(long)]
        out: Option<String>,
    },

    /// Replay a `gmr export` file into this repo's store. Refuses anything
    /// but a fresh store — this recreates history, it does not merge it.
    #[command(display_order = 41)]
    Import { file: String },
}

#[derive(Subcommand)]
pub enum ProbesCmd {
    /// Build, publish and install every recipe. Users get these prebuilt.
    Build,
    /// What each probe is, and the obs vocabulary it emits.
    List {
        #[arg(short, long)]
        verbose: bool,
    },
    /// Assemble what a release ships: recipes, pinned versions, artifacts.
    Bundle {
        #[arg(long)]
        out: String,
    },
}

#[derive(clap::Args)]
pub struct OpenArgs {
    pub key: String,
    /// The probe to look with, by name. `probes list` prints what this build knows.
    #[arg(long)]
    pub probe: String,
    #[arg(long, default_value = "{}")]
    pub params: String,
    #[arg(long = "rule", value_name = "GUARD => NEW_STATE")]
    pub rules: Vec<String>,
    #[arg(long = "terminal", value_delimiter = ',')]
    pub terminal: Vec<String>,
    /// Keep a full record even when the world did not move.
    #[arg(long)]
    pub retain_full: bool,
    /// This anchor's observation cadence, in seconds.
    #[arg(long)]
    pub cadence_secs: Option<u64>,
    /// Supersede an already-closed anchor. Closure is irreversible; correction opens a new generation.
    #[arg(long, requires = "why")]
    pub supersedes: Option<String>,
    #[arg(long, requires = "supersedes")]
    pub why: Option<String>,
}

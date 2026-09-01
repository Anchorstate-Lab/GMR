use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gmr",
    version,
    about = "Attach judgment to recomputable observations",
    long_about = "You write the memories; anchors are mechanical observations.\n\
                  Point at a coordinate, say what the code cannot say about itself,\n\
                  and this tool tells you when that position moves under you.",
    after_help = "The whole loop:\n\
                  \n  \
                  gmr init                       install probes\n  \
                  gmr anchor src/x.rs#name -m .. watch it, and write the memory\n  \
                  gmr status                     what am I watching\n  \
                  gmr check                      did anything I care about move?\n  \
                  gmr accept <key> --why ..      I looked; take this as the new baseline\n\
                  \n\
                  Other verbs exist for hand-driving the parts — `gmr help <name>`\n\
                  reaches them. Anything that changes criteria takes --why and seals it."
)]
pub struct Cli {
    #[arg(long, default_value = ".", global = true)]
    pub repo: String,

    #[arg(long, global = true)]
    pub json: bool,

    /// How long one round of observing may take, in milliseconds. A round is the whole
    /// operation, not each probe in it: `check` and `observe` look at one anchor per round,
    /// but `pass` observes a batch inside a single round, so this bounds the batch. Anchors
    /// the round never reaches are reported as skipped, not as failures.
    #[arg(long, global = true, default_value_t = gmr::Policy::default().probe_budget_ms)]
    pub probe_budget_ms: u64,

    /// How long one call to a memory store may take, in milliseconds. A store across a
    /// network is the only thing here that can hang, so this is the per-record bound.
    #[arg(long, global = true, default_value_t = gmr::Policy::default().content_call_ms)]
    pub content_call_ms: u64,

    /// How long reaching memory stores may take in total, in milliseconds, across every
    /// record one command looks at. Records whose turn never comes are reported as
    /// unreachable with a spent budget, never as missing.
    #[arg(long, global = true, default_value_t = gmr::Policy::default().content_total_ms)]
    pub content_total_ms: u64,

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

    /// Nominate what this repository already knows but never anchored:
    /// comments that read like constraints, documents that name real files.
    /// Prints one `gmr anchor` command per candidate and writes nothing —
    /// each line is a judgment for you to run or delete.
    #[command(display_order = 2)]
    Adopt {
        /// Files or directories to scan. Default: the whole repository.
        paths: Vec<String>,
        /// Skip comment blocks shorter than this many words.
        #[arg(long, default_value = "4")]
        min_words: usize,
    },

    /// Watch a coordinate and write the memory that goes with it. With no
    /// coordinate, open everything the declarations and notes already ask for.
    #[command(display_order = 1)]
    Anchor {
        /// `path` or `path#name`. The probe, shape and position follow from it.
        /// A `http://` or `https://` URL declares a fetched fact instead: the URL
        /// and the `#` selector are written into .anchor/probes.toml as a probe you
        /// can read and review, and the anchor is keyed by a short name, not the URL.
        coordinate: Option<String>,
        /// The name to key a fetched fact by. Without it a name is derived from the
        /// URL's last segment and the selector, and a collision is an error rather
        /// than a guess. Ignored for path coordinates, which are their own name.
        #[arg(long = "as")]
        named: Option<String>,
        /// The memory itself, written into this repository as a note. Reach for it
        /// when you keep no memories of your own: in git a note is the memory and
        /// the declaration in one file. If you already have a memory system, write
        /// the memory there and name it with --record instead.
        #[arg(short = 'm', long = "memory", conflicts_with = "record")]
        memory: Option<String>,
        /// A memory that already exists, by the address `gmr memories` prints
        /// (`<provider>:<id>`). The anchor is declared and this record is bound to
        /// it; nothing is written into any store.
        #[arg(long)]
        record: Option<String>,
    },

    /// What each memory store here will show you, and which of it is bound. Reads only.
    Memories {
        /// Only this store. Default: every store in this binary that can list.
        #[arg(long)]
        provider: Option<String>,
    },

    /// What is being watched, on which axes, with which memories. Reads only.
    #[command(display_order = 2)]
    Status { key: Option<String> },

    /// Did anything move on an axis a memory asked about? Exit 1 if so.
    #[command(display_order = 3)]
    Check { key: Option<String> },

    /// Record what you concluded, and what you were looking at when you did.
    /// A memory is a long-lived constraint someone reviewed; this is not that —
    /// it is one analysis's finding, held to the readings it was built from.
    #[command(display_order = 6)]
    Said {
        /// The conclusion, in your own words.
        text: String,
        /// The anchor it rests on. Repeat for several.
        #[arg(long = "on", required = true)]
        on: Vec<String>,
        /// The `fact_address` you were shown, as `gmr read <key> --json` printed
        /// it. Repeat for several. Leave it out and nothing records what you were
        /// looking at — which `standing` reports rather than assumes.
        #[arg(long = "saw")]
        saw: Vec<String>,
        /// One expression that is true while this conclusion still stands, over
        /// the anchors it names: `all(anchors, not state.v.sig)`.
        #[arg(long)]
        depends: Option<String>,
        /// Name it yourself. Default: a UTC timestamp.
        #[arg(long)]
        id: Option<String>,
    },

    /// Do the conclusions recorded here still stand? Exit 1 if any does not.
    /// `ground` is the contract's name for this question; `standing` remains
    /// as the older spelling of the same verb.
    #[command(display_order = 7, visible_alias = "standing")]
    Ground {
        /// One conclusion, by the id `said` printed. Default: all of them.
        id: Option<String>,
        /// Stop asking about this one. What it said stays in the table — an
        /// append-only record of what was believed — and nothing reads it again.
        #[arg(long, requires = "id")]
        retire: bool,
    },

    /// Write every anchor, every memory and what binds them as one HTML page.
    #[command(display_order = 5)]
    Atlas {
        /// Where the page goes. Defaults to .anchor/output/atlas.html, which
        /// git ignores along with the rest of .anchor/.
        #[arg(long)]
        out: Option<String>,
    },

    /// Build every declared probe recipe and install it for this machine.
    #[command(subcommand)]
    #[command(hide = true)]
    Probes(ProbesCmd),

    /// Open what the declarations and notes ask for, and align bindings.
    #[command(hide = true)]
    Sync {
        #[arg(default_value = crate::verbs::sync::DEFAULT_FILE)]
        file: String,
        #[arg(long)]
        dry_run: bool,
    },

    /// Open one anchor directly, naming its probe and rules by hand.
    #[command(hide = true)]
    Open(OpenArgs),

    /// Ask every anchor whether the world still matches. Exit 1 if any moved.
    #[command(hide = true)]
    Observe { key: Option<String> },

    /// One reading of an anchor, served fresh, with the address an answer
    /// built from it must cite. The cheap fold-only read: no memories are
    /// fetched.
    #[command(hide = true)]
    Sample {
        /// Anchor key, or `path:line` for a position.
        key: String,
        /// Look again first if the reading on record is older than this.
        #[arg(long)]
        fresher_than_secs: Option<u64>,
    },

    /// Each anchor's current state. A key names an anchor; `path:line` names
    /// a position, resolving to the anchor whose symbol starts at or above
    /// that line in that file, falling back to the file's own anchor.
    #[command(hide = true)]
    Read {
        key: Option<String>,
        /// Look again first if the last sighting is older than this many seconds.
        /// Unset, the stored reading is served whatever its age.
        #[arg(long)]
        fresher_than_secs: Option<u64>,
    },

    /// Accept what an anchor now shows: re-pin its baseline, or take the
    /// criteria its declaration changed. Needs --why, and the reason is sealed.
    #[command(display_order = 4)]
    Accept {
        key: Option<String>,
        /// Look again and pin what is there now as the new baseline.
        #[arg(long, conflicts_with = "criteria")]
        baseline: bool,
        /// Take the probe, rules or terminal the declaration now names.
        #[arg(long)]
        criteria: bool,
        /// Every anchor whose declaration moved. Only with --criteria: one
        /// declaration change is one decision, while every drifted baseline is
        /// a separate judgment that deserves its own reason.
        #[arg(long, requires = "criteria", conflicts_with = "key")]
        all: bool,
        #[arg(long)]
        why: String,
    },

    /// Install a directory as a named probe artifact.
    #[command(hide = true)]
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

    /// Change one criterion by hand: the probe, the transition table, the
    /// terminal set, or the state itself — exactly one. Needs --why, and the
    /// reason is sealed.
    #[command(hide = true)]
    Revise(ReviseArgs),

    /// Recapture against the instrument this build has. Needs --why, and the
    /// reason is sealed.
    #[command(hide = true)]
    Rebase {
        keys: Vec<String>,
        /// Every anchor standing on a reading a different instrument took.
        #[arg(long)]
        all: bool,
        #[arg(long)]
        why: String,
    },

    /// Say by hand which anchors a note is about; notes usually say it themselves.
    #[command(hide = true)]
    Bind {
        path: String,
        #[arg(long, value_delimiter = ',', conflicts_with = "detach")]
        anchors: Vec<String>,
        #[arg(long)]
        detach: bool,
        /// Which registered ContentProvider `path` is resolved through. Unset, a
        /// `<provider>:<id>` address picks its own and a bare path means `git`.
        /// What's actually available depends on how this binary was built.
        #[arg(long)]
        provider: Option<String>,
    },

    /// Say that a memory you just wrote is about these anchors. Records the
    /// assertion as self-attested: you wrote the record and you are vouching
    /// for it, and no reader should mistake that for a second opinion. Never
    /// needs the store to answer first — a record too fresh to be readable is
    /// exactly the moment the link is most accurate.
    #[command(hide = true)]
    Attest {
        /// `<provider>:<id>`, or the id the store just handed back with --provider.
        path: String,
        #[arg(long, value_delimiter = ',', required = true)]
        anchors: Vec<String>,
        /// Which registered ContentProvider `path` is resolved through.
        #[arg(long)]
        provider: Option<String>,
    },

    /// Re-stamp a binding's content version without changing which anchors it's about.
    #[command(hide = true)]
    Reaffirm {
        path: String,
        /// Which registered ContentProvider `path` is resolved through.
        #[arg(long)]
        provider: Option<String>,
    },

    /// Other references bound to any anchor `path` is also bound to.
    #[command(hide = true)]
    Cobound {
        path: String,
        /// Which registered ContentProvider `path` is resolved through.
        #[arg(long)]
        provider: Option<String>,
    },

    /// Record that `from` relates to `to`. Independent of anchoring — linking
    /// two references says nothing about which anchors either is bound to.
    /// The two ends may sit in different providers: a memory in one store can
    /// contradict a memory in another.
    #[command(hide = true)]
    Link {
        from: String,
        to: String,
        #[arg(long)]
        kind: String,
        /// Revoke every live assertion of this edge instead of adding one,
        /// whoever asserted it. The rows stay in the store; the revocation is
        /// what makes reads stop seeing them.
        #[arg(long)]
        detach: bool,
        #[arg(long)]
        from_provider: Option<String>,
        #[arg(long)]
        to_provider: Option<String>,
    },

    /// Retire an anchor. Closure is irreversible.
    #[command(display_order = 5)]
    Close {
        key: String,
        #[arg(long)]
        why: String,
    },

    /// Transitions, terminals and stalls since a point in the journal.
    /// `since` is the contract's name; `edges` remains as the older spelling.
    #[command(hide = true, visible_alias = "edges")]
    Since {
        #[arg(long, default_value = "0")]
        since: u64,
        #[arg(long)]
        status: Option<String>,
    },

    /// Per-anchor liveness: last seen, attempts, backoff.
    #[command(hide = true)]
    Health { key: Option<String> },

    /// Force `key` due now, clearing any backoff or parked state. Unlike
    /// `sync`, which only repairs a missing queue row, this always resets it.
    #[command(hide = true)]
    Requeue { key: String },

    /// Observe only what is due, and hand back the notes bound to whatever moved.
    #[command(hide = true)]
    Pass,

    /// Anchors that were never seen, or that carry no note.
    #[command(hide = true)]
    Doctor,

    /// Snapshot the journal, bindings, links and sealed rationale as JSONL,
    /// independent of this database's schema version. Run this with the old
    /// binary before upgrading — `import` on the new one replays it.
    #[command(hide = true)]
    Export {
        /// Where to write the JSONL. Defaults to stdout, so it can be piped.
        #[arg(long)]
        out: Option<String>,
    },

    /// Replay a `gmr export` file into this repo's store. Refuses anything
    /// but a fresh store — this recreates history, it does not merge it.
    #[command(hide = true)]
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
    /// A key, or a `path#name` coordinate when `--probe` is omitted.
    pub key: String,
    /// The probe to look with, by name. Omit it to route `key` as `path#name`.
    #[arg(long)]
    pub probe: Option<String>,
    /// What the probe is pointed at, as JSON. Omitted, a routed coordinate says it.
    #[arg(long)]
    pub params: Option<String>,
    /// A named transition preset, exclusive with `--rule`.
    #[arg(long, conflicts_with = "rules")]
    pub shape: Option<String>,
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
    /// How long this one anchor's probe may take, in milliseconds. Never widens --probe-budget-ms.
    #[arg(long)]
    pub budget_ms: Option<u64>,
    /// Supersede an already-closed anchor. Closure is irreversible; correction opens a new generation.
    #[arg(long, requires = "why")]
    pub supersedes: Option<String>,
    #[arg(long, requires = "supersedes")]
    pub why: Option<String>,
}

#[derive(clap::Args)]
pub struct ReviseArgs {
    pub key: String,
    /// Look somewhere else, by probe name.
    #[arg(long)]
    pub probe: Option<String>,
    #[arg(long, default_value = "{}")]
    pub params: String,
    /// Change what counts as a change.
    #[arg(long = "rule", value_name = "GUARD => NEW_STATE")]
    pub rules: Vec<String>,
    /// Change what is irreversible.
    #[arg(long = "terminal", value_delimiter = ',')]
    pub terminal: Vec<String>,
    /// Move the state directly, as a JSON object. Never derived from a
    /// declaration — there is no declared `state` facet to diff against, so
    /// this is the one always-manual form.
    #[arg(long)]
    pub state: Option<String>,
    #[arg(long)]
    pub why: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_default_has_one_source_of_truth() {
        let cli = Cli::parse_from(["gmr", "--repo", ".", "check"]);
        let policy = gmr::Policy::default();
        for (flag, from_cli, from_policy) in [
            (
                "--probe-budget-ms",
                cli.probe_budget_ms,
                policy.probe_budget_ms,
            ),
            (
                "--content-call-ms",
                cli.content_call_ms,
                policy.content_call_ms,
            ),
            (
                "--content-total-ms",
                cli.content_total_ms,
                policy.content_total_ms,
            ),
        ] {
            assert_eq!(
                from_cli, from_policy,
                "a literal here and a literal in Policy::default drift the moment one is \
                 edited, and main.rs hands {flag}'s value to the policy — so the policy's own \
                 default would quietly stop being what anyone gets"
            );
        }
    }
}

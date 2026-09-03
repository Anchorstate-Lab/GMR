/**
 * @anchorstate-lab/gmr — the verb table.
 *
 * These declarations describe `gmr.contract.v12.0`. That string is what a caller
 * pins to know which shapes they may match on: a contract type that changes
 * shape without it moving is a break they were told did not happen, and
 * tools/gate.py fails the build when the two disagree.
 *
 * The version has two segments: `v<breaking>.<additive>`. The additive segment
 * moves when an output type gains a variant or an optional field - survivable
 * by any consumer, under one rule this surface asks of every reader (the same
 * rule protobuf asks with unknown fields): **an exhaustive match over a
 * discriminant must keep a fallback arm**, because a runtime one additive step
 * ahead of these declarations may hand you a discriminant they do not name
 * yet. The breaking segment moves when anything is removed, renamed, or
 * re-meant - the only changes a fallback arm cannot survive. The same rule
 * holds for scripts parsing the CLI's `--json` output, whose binary upgrades
 * independently of anything pinning this file.
 *
 * Every refusal and failure is thrown with a message of the form
 * `<kind>: <message>`, where kind is a stable snake_case token — `refused`
 * (the console would not parse the ask), `assembly` (the runtime could not be
 * put together), `internal`, or a runtime code such as `not_bound`,
 * `no_such_anchor`, `no_provider`, `condensed_into_nothing`. Match the token,
 * not the prose.
 */
export const CONTRACT: "gmr.contract.v12.0";

/**
 * What a binding is about. `<provider>:<id>` names a record that lives in a
 * store; `said:<id>` names something an agent said, which lives nowhere —
 * the utterance is the claim.
 */
export type Address = string;
export type Seq = number;
export type Version = string;
export type FactAddress = string;
export type ProbeVersion = string;
export type Timestamp = string;

export interface Ref {
  provider: string;
  external_id: string;
}

/**
 * A stored record spells itself exactly as its `Ref` did before claims
 * existed, which is why every binding already written still reads.
 */
export type Claim = Ref | { said: string; asserts?: unknown };

/**
 * An invariant the asserter wrote down, in the expression language: true while
 * the claim still stands. It reads every anchor the claim names at once —
 * `all(anchors, …)`, `any(anchors, …)`, `count(anchors, …)` — and inside the
 * quantifier `state` is the one anchor being asked about.
 *
 * Opposite polarity from a subscription: a subscription fires when something
 * moved; this one goes quiet.
 */
export type Invariant = string;

export type Depends =
  | { depends: "holds" }
  | { depends: "broken" }
  /** Names anchors that were never opened here, so it cannot be answered. */
  | { depends: "ungrounded"; missing: string[] }
  /** Reads no anchor, so no state of the world could break it. */
  | { depends: "vacuous"; wrote: Invariant }
  | { depends: "unevaluable"; why: string }
  | { depends: "unstated" };

export type Openness =
  | "host_env" | "interpreter" | "network" | "clock" | "implementation" | "unknown";

export type Verifiability = "closed" | { open: { over: Openness[] } };

/**
 * What a probe reports. `named` lists the top-level fields it puts in `obs`;
 * a rule may read any path below one of them. `unknown` is the honest answer
 * for a probe that is somebody else's program.
 *
 * `open` refuses an anchor whose rules read a field a `named` probe never
 * reports — such an anchor observes forever, never transitions, and reads as
 * supervised the whole time.
 */
export type Observes =
  | { observes: "named"; fields: string[] }
  | { observes: "unknown" };

/** How a reading was derived. `observes` is absent when unknown. */
export interface Derivation {
  version: ProbeVersion;
  verifiability: Verifiability;
  observes?: Observes;
}

export type FailureCode =
  | "unreachable" | "timed_out" | "process_failed" | "unusable" | "artifact_invalid"
  | "output_too_large" | "invalid_json" | "unparseable" | "guard_not_boolean"
  | "new_state_not_an_object" | "new_state_absent" | "no_such_field" | "not_an_object"
  | "not_an_array" | "index_out_of_range" | "not_comparable" | "divided_by_zero";

export type ContentErrorCode = "provider_failed" | "budget_spent";

/**
 * What the record itself is doing, independently of any anchor. Under a
 * `lean` read `content` is absent: the reference and version are the
 * delivery, and a body worth reading is fetched by address.
 */
export type Grounding =
  | { grounding: "current"; version: Version; content?: string }
  | { grounding: "unverified"; version: Version; content?: string }
  | { grounding: "rewritten"; version: Version; content?: string; before: Before }
  | { grounding: "gone" }
  | { grounding: "no_provider"; provider: string }
  | { grounding: "unreachable"; code: ContentErrorCode; why: string };

/**
 * What a record is doing, in one word — the classifier `doctor` prints a line
 * for. `current` is the only one that needs nothing done about it.
 */
export type Footing =
  | "current" | "unverified" | "rewritten" | "no_before"
  | "gone" | "no_provider" | "unreachable" | "never_asked";

/**
 * A record this claim reaches by following links, that is not `current`.
 * `via` is the kinds traversed in order, so a reader can see which of this
 * memory's own citations led to the thing that moved.
 */
export interface Reached {
  reference: Ref;
  via: string[];
  depth: number;
  footing: Footing;
}

export type Before =
  | { before: "retrieved"; content: string }
  | { before: "not_retained" }
  | { before: "no_history" }
  /** A lean read asks history nothing, and says so rather than lying. */
  | { before: "not_asked" }
  | { before: "unreachable"; code: ContentErrorCode; why: string };

/** Whether the fact still stands where the memory was bound to it. */
export type Holding =
  | { holding: "holds" }
  /** The anchor finished; its journal is frozen and this can never change. */
  | { holding: "finished" }
  | { holding: "moved"; axes: string[]; at: Seq }
  | { holding: "incomparable"; took: ProbeVersion; reads: ProbeVersion }
  | { holding: "absent" }
  | { holding: "never_established" }
  | { holding: "undated" };

/** Whether we know, and if not, why not. A separate axis from `Holding`. */
export type Knowledge =
  | { knowledge: "seen"; at: Timestamp; verifiability: Verifiability }
  | { knowledge: "blind"; since: Timestamp | null; why: Blind };

export type Blind =
  | { blind: "never_asked" }
  | { blind: "unreachable"; code: FailureCode | null }
  | { blind: "unusable"; code: FailureCode | null }
  | { blind: "unevaluable"; code: FailureCode | null };

export interface Warrant {
  holding: Holding;
  knowledge: Knowledge;
}

/**
 * Was the asserter looking at this anchor's reading, or at a second
 * computation of the same fact running beside it? `saw` is what it cited;
 * `shown` is whether this anchor's journal actually holds that reading.
 */
export type Shown =
  | { shown: "seen"; at: Seq }
  /** A real reading, already replaced when the conclusion landed. */
  | { shown: "superseded"; at: Seq }
  | { shown: "unseen" }
  | { shown: "not_said" };

/** Addresses and versions, never values: follow them to audit the judgement. */
export type Evidence = {
  reading?: FactAddress;
  instrument?: ProbeVersion;
  bound_at?: Seq;
  moved_at?: Seq;
  /**
   * The readings the asserter was looking at — one per anchor it read, not
   * one per claim. Each anchor's `shown` asks whether any of them is a
   * reading it took.
   */
  saw?: FactAddress[];
} & Shown;

export type Anchored =
  | { anchored: "on"; key: string; warrant: Warrant; evidence: Evidence }
  | { anchored: "unopened"; key: string };

/** A turn asked about without being stored. `claim` is normally `said:<id>`. */
export interface Asked {
  claim: Address;
  anchors?: string[];
  /** Addresses of the readings it cited, as `sample` handed them back. */
  saw?: string[];
  asserts?: unknown;
  depends?: Invariant;
}

export type Standing = {
  claim: Claim;
  /** Absent for `said:` — an utterance is stored nowhere, so nothing to fetch. */
  record?: Grounding;
  /** The utterance this record condensed from, when it grew out of one. */
  origin?: string;
  on: Anchored[];
  /** Empty unless `reach` was asked for; only what is not `current`. */
  reached?: Reached[];
} & Depends;

/** One reading of an anchor, and the address an answer built from it must cite. */
export interface Reading {
  key: string;
  sighting: "found" | "absent";
  facts?: unknown;
  fact_address?: FactAddress;
  derivation?: Derivation;
  at: Timestamp | null;
  knowledge: Knowledge;
}

/** One edge as `read` delivers it: who it points at, what kind, who said so. */
export interface Linked {
  to: Ref;
  kind: string;
  source: Source;
  /** When the edge grew. Absent on rows older than the column. */
  at?: Timestamp;
}

/** An edge arriving at a record: who points at it, from the pointed-at end. */
export interface Inbound {
  from: Ref;
  kind: string;
  source: Source;
  at?: Timestamp;
}

/** Every live edge touching one record, both directions. */
export interface Links {
  out: Linked[];
  in: Inbound[];
}

/** An anchor's current state, as `read` frames it. */
export interface AnchorView {
  key: string;
  anchor: unknown;
  state: unknown;
  status: string | null;
  sighting: "found" | "absent";
  closed: boolean;
  faltering?: unknown;
  entered_at: Timestamp | null;
  last_sighting: Timestamp | null;
  sightings: number;
  derivation: Derivation | null;
  fact_address?: FactAddress;
  facts?: unknown;
}

/** One bound record, delivered with its warrant and grounding. */
export interface MemoryView {
  reference: Ref;
  /** The utterance this record condensed from, when it grew out of one. */
  origin?: string;
  bound_version?: Version;
  grounded: boolean;
  links: Linked[];
  bound_at_seq: Seq | null;
  baseline_at?: Seq;
  sources: Source[];
  asserted_at?: Timestamp;
  warrant?: Warrant;
  grounding: Grounding;
}

/**
 * A conclusion standing on the anchor: what an agent said, bound here, with
 * the same warrant a stored record gets. It has no body to fetch — the
 * utterance is the claim.
 */
export interface SaidView {
  id: string;
  asserts?: unknown;
  depends?: Invariant;
  sources: Source[];
  bound_at_seq: Seq | null;
  asserted_at?: Timestamp;
  warrant?: Warrant;
}

/**
 * The whole answer to `read`: the anchor flattened, plus its memories, plus
 * every conclusion asserted on it. `said` is absent when nothing was said.
 */
export type Grounded = AnchorView & { memories: MemoryView[]; said?: SaidView[] };

export type Edge =
  | { edge: "transitioned"; anchor: string; from: unknown; to: unknown;
      status: string | null; seq: Seq; at: Timestamp }
  | { edge: "closed"; anchor: string; self_sealed: boolean; seq: Seq; at: Timestamp }
  | { edge: "stalled"; anchor: string; count: number;
      last: "unreachable" | "unusable" | "unevaluable"; seq: Seq; at: Timestamp };

export type Raised =
  | { raised: "stale"; anchor: string; last_sighting: Timestamp | null }
  | { raised: "rewritten"; anchor: string; reference: Ref;
      bound_version: Version | null; current_version: Version; before: Before }
  | { raised: "gone"; anchor: string; reference: Ref; bound_version: Version | null }
  | { raised: "no_provider"; anchor: string; reference: Ref; provider: string }
  | { raised: "unreachable"; anchor: string; reference: Ref;
      code: ContentErrorCode; why: string };

export interface Edges {
  edges: Edge[];
  raised?: Raised[];
  cursor: Seq;
}

export interface Landed {
  anchors: string[];
  moved: [string, string][];
  recorded: boolean;
}

export interface Opened {
  key: string;
  state: unknown;
  warnings: string[];
  supersedes: string | null;
}

/**
 * What this call may spend and how fresh an answer it needs. Both spans are
 * milliseconds. An unknown field is refused, not dropped: a bound silently
 * ignored is an answer served stale under one the caller believes they set.
 */
export interface Instructions {
  max_staleness_ms?: number;
  budget_ms?: number;
  /**
   * How many link hops to follow from a stored claim, looking for records it
   * rests on that have themselves moved. Absent means not at all: a walk
   * nobody asked for is store reads nobody budgeted, on every call.
   */
  reach?: number;
  /**
   * Whether an anchor read also carries in records linked from its bound
   * ones, one hop, each marked `grounded: false` — bound elsewhere is not
   * about this anchor. Absent means only what is bound: same rule as
   * `reach`, the caller says whether to walk.
   */
  carry?: boolean;
  /**
   * Serve warrants and versions without record bodies. The envelope stays a
   * few hundred tokens however large the memories are; fetch a body by
   * address when a warrant makes it worth reading.
   */
  lean?: boolean;
}

/** Where a binding came from. `unknown` is how you say you do not know. */
export type Source = "derived" | "self_attested" | "adjudicated" | "configured" | "unknown";

/**
 * What an assertion carries besides "this claim is about these anchors". An
 * unknown field is refused, not dropped.
 */
export interface Asserting {
  /** The content version this assertion cites. Meaningless for `said:`. */
  bound_version?: Version;
  /**
   * The readings the asserter was looking at, as `sample` handed them back.
   * One per anchor read: a claim on four anchors looked at four readings.
   */
  saw?: FactAddress[];
  /** What a `said:` claim asserted. Recorded, never interpreted. */
  asserts?: unknown;
  /** True while the claim still stands. */
  depends?: Invariant;
}

export interface Rule {
  when: string;
  to: string;
}

export interface RunSettings {
  retain?: "tick" | "full";
  facts?: "plain" | "digests";
  cadence_secs?: number;
  budget_ms?: number;
}

export interface OpenRequest {
  key: string;
  probe: { kind: string; name: string; params?: unknown };
  transitions?: Rule[];
  terminal?: string[];
  initial?: unknown;
  settings?: RunSettings;
  supersedes?: { key: string; rationale: string };
}

export interface Recipes {
  http?: Record<string, {
    url: string;
    select?: string;
    headers?: Record<string, { given: string } | { from_env: string }>;
  }>;
  file?: Record<string, {
    path: string;
    select?: string;
    shaped?: "json" | "toml" | "yaml";
  }>;
  sql?: Record<string, {
    source: { given: string } | { from_env: string };
    query: string;
    column?: string;
    binds?: string[];
  }>;
}

export interface Providers {
  git?: boolean;
  claude_code?: boolean;
  mem0?: boolean;
}

export interface Policy {
  cadence_secs?: number;
  lease_secs?: number;
  backoff_base_secs?: number;
  backoff_cap_secs?: number;
  batch?: number;
  observe_at_once?: number;
  stalled_attempts?: number;
  stalled_staleness_secs?: number;
  probe_budget_ms?: number;
  probe_output_cap?: number;
  content_call_ms?: number;
  content_total_ms?: number;
}

export interface Opening {
  /** What `file`, `script` and `shell` probes are relative to. */
  root: string;
  /** Defaults to `<root>/.anchor/state/memory.db`, where the CLI keeps it. */
  db?: string;
  recipes?: Recipes;
  /** Probe name to script path, relative to `root`. */
  scripts?: Record<string, string>;
  providers?: Providers;
  policy?: Policy;
}

export class Gmr {
  /**
   * Do these sentences still stand? One answer per ask, in order. An address
   * asks about what the store holds; an object asks about a turn nobody stored,
   * and writes nothing. A claim that was asserted refuses the object form.
   */
  ground(claims: (Address | Asked)[], how?: Instructions): Promise<Standing[]>;
  /**
   * Read an anchor and hand back what it sees, with the address of that
   * reading. Build the answer from `facts`, then cite `fact_address` as
   * `saw` when binding it — that is what makes the answer and the anchor
   * the same look at the world rather than two.
   */
  sample(anchor: string, how?: Instructions): Promise<Reading>;

  /**
   * The full envelope for one anchor: its state plus every bound record with
   * warrant and grounding. `how.carry` opts into linked records, each marked
   * `grounded: false` — bound elsewhere is not about this anchor.
   */
  read(anchor: string, how?: Instructions): Promise<Grounded>;
  /** What changed after this point in the journal. */
  since(cursor: Seq, status?: string): Promise<Edges>;
  /** This sentence is about these anchors. */
  bind(claim: Address, anchors: string[], source: Source,
       how?: Asserting): Promise<Landed>;
  /** It is not any more. Answers with the anchors it was cleared from. */
  revoke(claim: Address, source: Source): Promise<string[]>;
  /** Open an anchor. */
  open(request: OpenRequest): Promise<Opened>;
  /** Retire one. Closure is irreversible. */
  close(key: string, why: string): Promise<void>;

  /** Assert a typed edge between two stored records, with its provenance. */
  link(from: Address, to: Address, kind: string, source: Source): Promise<void>;

  /**
   * Revoke every live assertion of this edge — or, with `asserted_as`, only
   * the assertions carrying that provenance; returns how many rows the
   * revocation named. The rows stay — reads stop seeing them.
   */
  unlink(from: Address, to: Address, kind: string, source: Source,
         asserted_as?: Source): Promise<number>;

  /** The roster: one sample-shaped reading per anchor. The walk's first hop. */
  anchors(): Promise<Reading[]>;
  /** Every claim the store holds a live binding for, records and utterances both. */
  claims(): Promise<Claim[]>;
  /** Every claim sharing an anchor with this one — utterances included. */
  cobound(claim: Address): Promise<Claim[]>;
  /** Every live edge touching a record, from both ends. */
  links(record: Address): Promise<Links>;
  /**
   * An utterance became a memory: bind the record on every anchor the
   * `said:` claim stood on, carrying its `saw` and `depends` and recording
   * the utterance as `origin`, then revoke the utterance. The lineage
   * survives in the new binding.
   */
  condense(said: Address, into: Address, source: Source): Promise<Landed>;
}

export function open(options: Opening): Promise<Gmr>;

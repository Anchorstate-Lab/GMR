/**
 * @anchorstate-lab/gmr — the seven verbs.
 *
 * These declarations describe `gmr.contract.v3`. That string is what a caller
 * pins to know which shapes they may match on: a contract type that changes
 * shape without it moving is a break they were told did not happen, and
 * tools/gate.py fails the build when the two disagree.
 */
export const CONTRACT: "gmr.contract.v3";

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

export type Openness =
  | "host_env" | "interpreter" | "network" | "clock" | "implementation" | "unknown";

export type Verifiability = "closed" | { open: { over: Openness[] } };

export type FailureCode =
  | "unreachable" | "timed_out" | "process_failed" | "unusable" | "artifact_invalid"
  | "output_too_large" | "invalid_json" | "unparseable" | "guard_not_boolean"
  | "new_state_not_an_object" | "new_state_absent" | "no_such_field" | "not_an_object"
  | "not_an_array" | "index_out_of_range" | "not_comparable" | "divided_by_zero";

export type ContentErrorCode = "provider_failed" | "budget_spent";

/** What the record itself is doing, independently of any anchor. */
export type Grounding =
  | { grounding: "current"; version: Version; content: string }
  | { grounding: "unverified"; version: Version; content: string }
  | { grounding: "rewritten"; version: Version; content: string; before: Before }
  | { grounding: "gone" }
  | { grounding: "no_provider"; provider: string }
  | { grounding: "unreachable"; code: ContentErrorCode; why: string };

export type Before =
  | { before: "retrieved"; content: string }
  | { before: "not_retained" }
  | { before: "no_history" }
  | { before: "unreachable"; code: ContentErrorCode; why: string };

/** Whether the fact still stands where the memory was bound to it. */
export type Holding =
  | { holding: "holds" }
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
  | { shown: "unseen" }
  | { shown: "not_said" };

/** Addresses and versions, never values: follow them to audit the judgement. */
export type Evidence = {
  reading?: FactAddress;
  instrument?: ProbeVersion;
  bound_at?: Seq;
  moved_at?: Seq;
  saw?: FactAddress;
} & Shown;

export type Anchored =
  | { anchored: "on"; key: string; warrant: Warrant; evidence: Evidence }
  | { anchored: "unopened"; key: string };

export interface Standing {
  claim: Claim;
  /** Absent for `said:` — an utterance is stored nowhere, so nothing to fetch. */
  record?: Grounding;
  on: Anchored[];
}

/** One reading of an anchor, and the address an answer built from it must cite. */
export interface Reading {
  key: string;
  sighting: "found" | "absent";
  facts?: unknown;
  fact_address?: FactAddress;
  derivation?: { kind: string; name: string; version: ProbeVersion;
                 verifiability: Verifiability };
  at: Timestamp | null;
  knowledge: Knowledge;
}

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
}

/** Where a binding came from. `unknown` is how you say you do not know. */
export type Source = "derived" | "self_attested" | "adjudicated" | "configured" | "unknown";

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
  /** Do these sentences still stand? One answer per claim, in order. */
  ground(claims: Address[], how?: Instructions): Promise<Standing[]>;
  /**
   * Read an anchor and hand back what it sees, with the address of that
   * reading. Build the answer from `facts`, then cite `fact_address` as
   * `saw` when binding it — that is what makes the answer and the anchor
   * the same look at the world rather than two.
   */
  sample(anchor: string, how?: Instructions): Promise<Reading>;
  /** What changed after this point in the journal. */
  since(cursor: Seq, status?: string): Promise<Edges>;
  /** This sentence is about these anchors. */
  bind(claim: Address, anchors: string[], source: Source,
       boundVersion?: Version, saw?: FactAddress,
       asserts?: unknown): Promise<Landed>;
  /** It is not any more. Answers with the anchors it was cleared from. */
  revoke(claim: Address, source: Source): Promise<string[]>;
  /** Open an anchor. */
  open(request: OpenRequest): Promise<Opened>;
  /** Retire one. Closure is irreversible. */
  close(key: string, why: string): Promise<void>;
}

export function open(options: Opening): Promise<Gmr>;

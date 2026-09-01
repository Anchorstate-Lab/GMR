# GMR

*A Grounding Architecture for Evolving Memory*

## Abstract

Long-lived agent memory creates a reliability problem that retrieval alone does not solve. A memory is usually a judgment formed under particular conditions, while the code, interfaces, configurations, documents, and behaviours that gave rise to that judgment continue to change. If the memory is retained as an ordinary document, the system may retrieve it after its supporting conditions have changed and provide no indication that the judgment deserves review. The result is not merely stale data. It is a silent change in the meaning of a decision.

GMR, the Grounded Memory Runtime, addresses this problem by introducing an anchoring layer between external observation and memory retrieval. An Anchor declares what must be observed, how an observation is interpreted as a state transition, and which memories are related to that interpretation. Probes observe external reality; versioned Observations preserve the identity of what was observed; a deterministic transition language interprets those observations; and an append-only Journal preserves the history from which the current Anchor state is projected. When the declared semantics indicate that a judgment needs to be reconsidered, the domain-facing runtime can surface the relevant memory with its bound version and current provider version.

The same anchoring layer answers a second question about a different kind of judgment. A memory is a long-lived constraint that somebody wrote and somebody reviewed; an inference is what one analysis concluded for the task in front of it. They fail differently — a memory drifts and is handed back to a person, while an inference simply loses its ground — so the runtime records what an inference rests on at the moment it is made: the anchors it is about, the exact reading its author was shown, and an invariant its author states is true while the conclusion still stands. Asking about it later reports whether the anchor's ground has moved, whether the cited reading is one that anchor ever took, and whether the stated invariant still holds.

The central architectural claim is deliberately narrower than “GMR keeps memories true”. GMR cannot determine whether a probe observes the right thing, whether a rule expresses the right judgment, or whether an agent should accept the new situation. It reports **structure, never entailment**: that a claim is bound to these anchors, that they read differently now than when it was bound, that it cited a reading no anchor took, that its author's invariant has failed. It does not conclude that the claim is therefore false. Structure is recomputable by a third party from the journal, the store and the probe; entailment is not, and a component that decided it would be the one unauditable part of a system whose whole argument is auditability. The runtime therefore provides a reliability layer for declared grounding relationships, not a truth engine.

This document presents the design as a coherent system rather than as a package catalogue. It defines the problem, the domain model, the observation and transition semantics, the journal and memory protocols, the concurrency model, the implementation mapping in this repository, and the limits that a future extension must not obscure.

### On this document's standing

`GMR.md` is the source of truth for the architectural decisions themselves, and it is written under a rule this document does not follow: it names no type, field, function, file, or call path, on the grounds that a document which names them begins to rot the moment the code moves, and rots in a specific way — a reader finds an interface described here that no longer exists, takes it for the present, and designs against it. This document accepts that cost in exchange for being concrete enough to argue with. The exchange only holds under two conditions.

Where the two disagree about a decision, `GMR.md` is right and this document has drifted. Where this document describes the implementation, the code is right and this document is a claim about the code, not evidence of it. Read §14 in that spirit: it is a map from responsibilities to their current addresses, useful for finding things and worthless as proof that they are correct. Nothing here should be trusted over a probe, and no mechanically checkable constraint stated here belongs here rather than in the gate.

## 1. Introduction

### 1.1 The motivation

Consider a system in which an engineer records the following judgment:

> Keep the request timeout below thirty minutes because long-running requests exhaust the worker pool.

The sentence is not itself a fact. It is a judgment about a fact. It contains a reason, a constraint, and an intended response to a future situation. At the time it is written, the timeout may be thirty minutes and the worker pool may have a particular size. Later, the timeout may become two hours, the worker pool may be replaced, or the request path may move to a different service. The text of the memory can remain perfectly intact while its grounding relationship has changed.

Traditional memory systems are good at preserving and retrieving the sentence. They are not, by themselves, responsible for determining when the conditions under which the sentence was written have changed. A vector index can return a semantically similar note; a document store can preserve its bytes; a knowledge graph can record an explicit relation. None of those mechanisms, without an additional temporal and observational contract, establishes that a judgment should be reconsidered because an observable dependency crossed a meaningful boundary.

GMR exists to make that contract executable.

### 1.2 The design thesis

The design places an Anchor between memory and fact:

```text
  Memory              a human or agent judgment
     │
     │ binding: this judgment is about this relationship
     ▼
  Anchor              the observable criterion and its interpretation
     │
     │ probe invocation
     ▼
  Observation         a versioned account of what was seen
     │
     │ transition evaluation
     ▼
  Anchor state        the domain's durable interpretation over time
     │
     │ domain delivery policy
     ▼
  Memory resurfaced    the judgment becomes available for review
```

This indirection is the essential design decision. A direct `Memory → Fact` relation is too weak because it says neither which instrument produced the fact nor what kind of change makes the judgment relevant again. A direct `Memory → Rule` relation is too strong because it makes the memory responsible for interpreting observations and encourages the memory layer to become a second, implicit runtime. The Anchor owns the relationship while leaving the meaning of the judgment to the memory's author.

### 1.3 What GMR guarantees

For a declared Anchor, GMR provides a chain of durable evidence:

1. the Anchor records the declared probe and transition criteria;
2. a successful observation records its outcome, fact address, declaration identity, derivation identity, and evaluator identity;
3. the Journal records the ordered history of observations, transitions, failures, revisions, and closure;
4. the current state is reconstructed from the Journal rather than maintained as an unverified mutable cache;
5. a bound memory can be compared with the version seen at bind time and, where the provider permits it, with the content that existed then;
6. a bound claim carries what its author was looking at and what its author said kept it standing, so a later reader can be told whether that reading was one the Anchor took and whether that condition still holds;
7. a domain can decide, from the resulting state, whether the memory should be handed back to a human or an agent.

These guarantees are conditional on the declaration. GMR does not know whether the chosen probe observes the correct object, whether the probe's output is a complete representation, whether a transition rule expresses a sound policy, or whether the memory's author made a good decision. Determinism is not objectivity, and content addressing is not truth.

### 1.4 What GMR does not guarantee

GMR does not guarantee that every change in external reality is observed. A change outside the selected probe's field of view is outside the Anchor's contract. It does not guarantee that every change in an observation resurfaces every related memory. The current implementation distinguishes a changed raw observation from a state transition, and then lets the coding domain decide delivery from the resulting state rather than from the fact that a transition occurred. A fact can change without the interpreted state changing, and such a change is not automatically a memory delivery event.

GMR does not decide entailment. Nothing in the substrate compares a claim's content against a reading, and no reported field folds “the ground moved”, “the cited reading was never taken” and “the stated invariant failed” into a single verdict about whether the claim may still be relied upon. Those three are simultaneously true or false of different things, and the reduction that turns them into advice is a policy belonging to whoever is about to act. A domain may adopt one; the substrate does not ship one.

GMR also does not automatically rewrite memory. Resurfacing is a request for review, not an autonomous decision that the original judgment is false. A caller may accept a new baseline, revise the Anchor's criteria, reaffirm the memory binding, write a new memory version, or close the Anchor. Those are different judgments and are therefore different operations in the architecture.

## 2. The problem as a systems problem

### 2.1 Memory is a judgment, not a fact cache

A useful memory commonly contains at least four kinds of information: an observation that was available at the time, an interpretation of that observation, a reason for the interpretation, and an intended consequence. The first part may look factual, but the whole record is not reducible to a fact payload. If GMR copied the observed payload into every memory, it would create two competing fact stores. One would be updated by probes and the other would be updated, if at all, by an author or an agent. Their divergence would be precisely the problem the system is intended to reveal.

GMR therefore treats memory as external content. The runtime stores a stable provider and external identifier, the relationship to one or more Anchors, the version seen when that relationship was recorded, and the historical Anchor sequence when a single Anchor makes such a sequence meaningful. The provider remains the authority for the memory body.

### 2.2 The failure mode is temporal and semantic

The dangerous state is not simply “the memory has old bytes”. A memory may be old and still valid. The dangerous state is that the system retrieves a judgment as if its applicability were unchanged even though the observable conditions that constrained the judgment have moved.

This creates two requirements that are often conflated. The first is temporal: the system must preserve which observation and which memory version existed at a particular point. The second is semantic: the system must define which changes matter for the judgment. Versioning solves the first problem; Anchors and transition rules solve the second. Neither can replace the other.

### 2.3 Requirements derived from the failure mode

The architecture follows from the failure mode rather than from a preferred storage technology.

An observation must have an identity that changes when the implementation that derived it changes, even if the resulting payload happens to be equal. A missing target must be represented as an observed outcome rather than as an absence of an observation, because “the target was checked and not found” is different from “the target was not checked”. A rule failure must not be treated as a negative fact, because “the condition could not be evaluated” is not evidence that the condition is false. Historical state must be append-only, because a mutable current row cannot explain which criteria produced a prior decision. Finally, criteria changes must be explicit, because silently changing the interpretation of history changes the meaning of the history itself.

### 2.4 The correct unit of reliability

The unit of reliability is not a memory, a fact, or a probe in isolation. It is the *grounding relationship*:

```text
  a judgment
  + a declared observable target
  + a derivation identity
  + an interpretation function
  + a history of observations and decisions
```

An Anchor is the durable representation of that relationship. Its design is successful when a future reader can answer not only “what does the memory say?” but also “what did this memory depend on, what instrument looked at it, what changed, how was that change interpreted, and which human judgment accepted the current baseline?”

## 3. Domain model

### 3.1 Two different worlds

GMR spans two worlds but does not merge them. The external world contains the objects and behaviours being observed. The internal world contains declarations, observations, state, history, memory references, and decisions about criteria.

The external world is open-ended and may be unavailable. The internal world is structured and journaled. A probe is the controlled interface between them. The fact that a probe is deterministic means that the same derivation and the same target can produce the same result; it does not mean that the result is correct or that the target itself is immutable.

### 3.2 Reality and position

Reality is not represented in `gmr-core`. The core does not know that a probe can be invoked at all: invocation is the seam `gmr-probe` defines, and the core holds only the vocabulary an Anchor needs to name a probe and record its answer. What the core knows about the target is narrower still — that the Anchor state has a `position` slot and where to read it. The position lives in state rather than in the probe declaration because the domain may need to move the target as part of a transition, and the substrate does not interpret the structure of that JSON value.

This is an important separation. A coding domain may use a fuzzy coordinate that contains a file, symbol kind, name, visibility, or structural shape. Another domain may use an endpoint, a configuration path, or a document heading. The substrate must not encode those domain meanings as a universal coordinate type.

The coding probes intentionally use fuzzy coordinates rather than line numbers or other brittle addresses. A coordinate can report that the name matched while the file did not, or that the shape matched while the name did not. That partial evidence allows the domain's state machine to distinguish a move, a rename, a contract change, and a disappearance without asking the substrate to understand source code.

### 3.3 Probe declaration and execution

A probe declaration is a value containing a kind, a name, and parameters. It answers the question “what is the Anchor asking for?” The `Transport` interface answers a different question: “which implementation can resolve and execute that declaration here?”

The separation is necessary because a name is not a version. A declared name may resolve to a different implementation after an installation or deployment change. The runtime records the derivation returned by the transport at the time of invocation rather than assuming that the declaration describes the implementation completely.

### 3.4 Outcome and facts

The probe contract has two successful outcomes. `Found` carries a JSON fact payload. `NotFound` carries no payload but remains a meaningful answer. The contract also has failure outcomes represented by structured `ProbeError` values. A timeout, process failure, invalid artifact, oversized output, or invalid JSON is a failure of the observation mechanism, not a statement about the external world.

This distinction is the first protection against hallucination. If a probe cannot run, the runtime records an `Attempt` and preserves the last successful world reading. It does not replace uncertainty with a convenient “missing” fact.

### 3.5 Observation and fact address

An Observation is the successful result of a probe invocation. In the current implementation it has three conceptual parts:

```text
Observation = (Outcome, FactAddress, Versions)
```

The `Outcome` preserves the facts themselves. The `FactAddress` identifies the outcome under the derivation version that produced it. `Versions` preserves the declaration hash, the full derivation record, and the evaluator version.

The `FactAddress` is calculated over the derivation version, the found/not-found flag, and the canonical fact payload. The derivation's verifiability field is retained in `Versions` but is not currently an input to the address. This distinction is small in code and large in meaning: the address answers “which version and answer did this observation have?”, while the `Derivation` record also answers “what level of replayability does the transport claim?”

### 3.6 Anchor and state

An Anchor contains its key, probe declaration, ordered transition rules, terminal status set, and optional supersession metadata. It is a policy object, not a memory object and not a fact object.

State is a JSON value owned by the domain, in practice an object. The substrate carries it, compares it for equality, reads its position slot, and checks its status against the Anchor's terminal set. It does not define the meaning of other fields and does not provide a fixed vocabulary of statuses. This permits a coding domain to represent signature drift, implementation drift, movement, or missing targets, while a different domain may represent settlement, expiration, or recovery.

Only one point enforces the object shape, and it is the one that matters: a transition rule's new state must evaluate to an object, so no observation can move an Anchor onto a state that has no addressable fields. Elsewhere the type is permissive, and reading its slots on a state that has none yields absence rather than an error — the substrate must be able to carry a state it does not understand, including an empty one.

State may contain an accumulator. If a domain needs a count, a baseline, a vector of active axes, or a recovered position, the domain encodes that value in state and writes rules that carry it forward. This is more explicit than hiding an accumulator in a substrate-level “previous observation” facility, because it makes the accumulation rule part of the Anchor's auditable criteria.

### 3.7 Claim, reference, and binding

What binds to an Anchor is a `Claim`, and it comes in two shapes. A stored claim is a `Ref`: a provider identity and an external identifier, whose content lives in somebody else's store and can be fetched — currently, and where the provider supports it, at a requested historical version. An uttered claim is a `Said`: an identifier and the sentence itself, which lives nowhere else. The utterance *is* the claim, so there is no document to fetch and no content version to compare, and a report about one says so rather than reporting a fetch failure.

The core `Binding` relation connects a claim to one or more Anchor keys and, optionally, to an invariant its author states. It answers “what is this claim about, and what did its author say keeps it standing?” A `BindingRecord` adds the occasion: the version observed when the relation was recorded, a binding-time sequence when one Anchor gives that sequence a unique meaning, the fact addresses its author was looking at, and the source that vouches for the relation. Keeping these separate allows rebinding to append a new occasion without changing the identity of the relation itself.

`Source` is the axis that says who is speaking. `Derived` and `Adjudicated` are the repository speaking — a note declared what it is about, or a person said so. `SelfAttested` is an agent vouching for a record it wrote itself. That is worth recording rather than smoothing over: it is not a second opinion, and a reader is shown which it is.

### 3.8 Journal and projection

The Journal is the authoritative historical sequence for an Anchor. The core entry algebra contains `Open`, successful observation entries (`Transition` and `Still`), `Attempt`, `Revise`, and `Close`.

The current state is a projection of those entries. The `fold` operation walks the ordered sequence and reconstructs the current Anchor, state, latest observation, attempt streak, timestamps, revision counts, and closure bit. A caller that stores a second mutable current-state representation would be creating a second source of truth and would weaken replayability.

### 3.9 Events and standing conditions

An event happened at a journal sequence. A standing condition is true at query time and may remain true after it has been reported. GMR keeps these categories separate. The `changed_since` operation uses a journal cursor for events while recomputing standing conditions from current provider state and the deployment's staleness policy.

There are three kinds of event, not two. A transition and a closure are the obvious ones. The third is a stall, and it exists because the two ways an observation can fail do not deserve the same patience. An unreachable world is worth retrying, so a stall is reported only once the attempt streak reaches the policy threshold. A rule that cannot be evaluated is deterministic — repeating it ten thousand times will not make it evaluable — so it emits a stall on the first attempt. Collapsing the two would let a mistyped rule sit quietly inside a streak counter that was designed for flaky networks.

The standing conditions are that an Anchor has not been sighted within the staleness window, and that a bound memory's current version no longer matches the version recorded at bind time. Neither is answerable from the Journal, which is why neither can carry a cursor.

### 3.10 Memory and inference are two layers over one mechanism

A memory is a long-lived constraint on a fact: written by a person, reviewed in a commit, and not derivable from the fact, because a constraint that can be derived is one nobody needed to write. An inference is what one analysis concluded for the task in front of it — one occasion, not yet condensed into anything.

They share the Anchor and they fail differently. A memory *drifts*: the code moved and whether what was written still holds is now unknown. An inference *loses its ground*: the reading it rested on moved, or it was never built from that reading at all.

The processing differs accordingly, and this is the part that is easy to get wrong. A memory whose coordinate moved is not false, it is due — the verdict needs a person, which is why the coding domain's check hands it back and acceptance seals a reason. An inference whose stated condition failed needs nobody: the sentence is no longer supported and there is nothing to re-read.

```text
memory     check  → handed back → a person re-reads → accept with a sealed reason
inference  ground → holding, shown, depends → the caller decides
```

Where a criterion lives follows from which layer it belongs to. A memory's criteria live in the repository, in the file, read fresh, so that editing them takes effect on the next check with no build step between the author and the checker. An inference's criteria live in the append-only binding record, unchangeable, because they are not a source of truth to be re-read but evidence of what was believed at the time.

The two must not be built out of each other. Compiling a note's subscription into a binding invariant would leave the checker comparing code against a copy of the memory taken at the last synchronization rather than against the memory — manufacturing, inside the checker, the one drift the system exists to catch. Keeping an inference as if it were a memory promotes a one-occasion conclusion into a constraint nobody reviewed. Their polarities differ for the same reason and it is not an inconsistency to be tidied away: a subscription is true when the memory must come back, while an invariant is true while the claim still stands.

## 4. Formal grounding semantics

This section states the semantic core independently of Rust module names. The purpose is not to replace the implementation with mathematics, but to make clear which distinctions are architectural and which are incidental.

### 4.1 Observation identity

Let an outcome be either `Found(f)` for a canonical JSON value `f`, or `NotFound`. Let `d` be the derivation version that the transport resolved for the invocation. The fact identity is:

```text
A(outcome, d) = H({
    "derivation": d,
    "found": outcome is Found,
    "facts": f when outcome is Found, null otherwise
})
```

`H` is the repository's canonical content hash. Consequently, the following identities are intentionally different:

```text
A(Found(f), d1)     ≠ A(Found(f), d2)
A(Found(f), d)      ≠ A(NotFound, d)
A(NotFound, d1)     ≠ A(NotFound, d2)
```

The first relation prevents an implementation swap from disappearing merely because the new implementation happened to emit equal facts. The second relation distinguishes an observed absence from a found value. The third relation distinguishes “the new instrument also observed absence” from “the instrument never ran”.

The declaration hash is not substituted for `d`. A declaration describes what the Anchor requested; the derivation describes what actually produced the outcome. Collapsing them would allow a probe implementation to change while the historical identity falsely remained stable.

### 4.2 Transition semantics

Let an Anchor be `a`, an Observation be `o`, the current state be `s`, the time of observation be `t`, and the time at which the current state was entered be `e`. The transition evaluator computes:

```text
T(a, o, s, t, e) = To(s') | Unchanged | Unevaluable(error)
```

The evaluator exposes the observation's facts as `obs`, the current domain state as `state`, and the two recorded times as `taken_at` and `entered_at`. It has no ambient clock, I/O capability, random source, or provider access.

The Anchor contains an ordered list of pairs `(guard_i, body_i)`. Evaluation proceeds from the first pair to the last. A guard that is false or absent does not match and evaluation continues. The first guard that evaluates to the boolean value true selects its body. If no guard matches, the result is `Unchanged`.

Absence means opposite things in the two positions, and the asymmetry is deliberate. A guard that evaluates to absent is undecidable rather than wrong, so its rule is skipped and a later rule gets its turn — a rule that cannot yet be decided must not consume the decision. A body that evaluates to absent has been selected and then cannot produce the state it promised, which is a fault. So an absent guard yields `Unchanged` if nothing else matches, while an absent body yields `Unevaluable`.

The selected body must evaluate to a JSON object, and that object is the complete next state. Anything else fails with a structured code: a scalar or array where a state was required, an absent new state, an unparseable expression in either position, a guard that evaluated to a non-boolean, a missing observation field, an invalid path, an incomparable comparison, or a division by zero. The evaluator never interprets a fault as false.

State paths are intentionally more lenient than observation paths. A missing state field can be absent because the domain has not accumulated it yet; a missing observation field indicates that a rule asks the instrument for a direction it did not provide. The coding domain checks this contract at sync time where possible and the runtime records a failure if an actual evaluation still becomes impossible.

### 4.3 Journal semantics

Let `J_a` be the ordered entries of Anchor `a`. The projection is:

```text
S_a = Fold(J_a)
```

An `Open` entry establishes the initial Anchor, observation and state. A `Transition` entry replaces the projected latest observation and state. A `Still` entry records that a new observation was compared but can refer back to an earlier full sighting. An `Attempt` increases the failure streak without changing the last successful world reading. A `Revise` changes criteria or state as an explicit author action. A `Close` sets the closure bit.

Closure is monotonic:

```text
closed_next = closed_previous OR entered_declared_terminal_state OR explicit_close
```

The OR is a historical fact. If a later state revision moves the state out of a terminal set, the Anchor remains closed because it did enter a terminal state. Recomputing closure only from the latest state would erase the fact that closure had occurred and would permit accidental resurrection.

### 4.4 Observation compaction

Suppose the previous projected state is `s`, its latest fact address is `a`, the new transition result is `s'`, and the new fact address is `a'`. The runtime may write a `Still` entry exactly when:

```text
s = s' AND a = a'
```

If the state is equal but the fact address changed, a full `Transition` entry is written even though the result is reported as `Unchanged` to the caller. This preserves the fact that the world was checked under a new observation identity. The `retain: full` setting disables this compaction for deployments that need a full record for every successful sighting.

### 4.5 Memory delivery semantics

Memory delivery is a function of the domain, not a primitive fact of the core:

```text
D(domain_policy, memory, state, moved) → deliver | do_not_deliver
```

In the coding domain, named shapes project a state vector into axes such as signature, logic, location, or missing. A memory's `watch` declaration selects which axes can deliver it. A shaped Anchor can move on an unwatched axis without delivering a note, although the movement remains visible through status and read operations.

The `moved` argument is not the primary input, and this is the load-bearing part of the contract. For a shaped Anchor, delivery is **level-triggered**: it asks whether any subscribed axis is currently set, not whether it was set by this observation. The axis bits accumulate in state and come down only when a human accepts a new baseline, so a memory keeps being handed back on every subsequent run until someone settles it. Settled is derived rather than listed — every bit down — because an enumerated list of settled statuses would be a second answer to the same question, and one of two answers always goes un-updated. `moved` is consulted only in the fallback case, where the Anchor has hand-written rules that no named shape recognises; nothing there declares what counts as settled, so level triggering would mean “never green”, and the domain falls back to the transition edge. That fallback is a known and deliberate downgrade, not an oversight.

An edge-triggered delivery rule is the tempting simplification here, and it was the original implementation. Its measured failure was that a second observation over an unrepaired Anchor reported nothing and exited zero, so a broken Anchor could sit in a green build indefinitely. Any future change that reduces the shaped path to “was this the observation that moved it” reintroduces exactly that.

This yields a precise distinction:

```text
fact drift       a new successful observation identity
state transition the Anchor's interpretation changed
memory delivery  the domain says this state still needs a human
```

The current system guarantees the second and third only according to declared rules and subscriptions. It does not claim that every raw fact-address change must deliver every bound memory. A product that needs that stronger behaviour must make fact identity a subscribed axis or extend the delivery contract explicitly.

### 4.6 Grounding a claim

Delivery answers a domain's question about a memory. Grounding answers a caller's question about a claim it is holding: a sentence it just said, or one it is about to. The operation is keyed by claim, because that is what a caller has, and it reports four things that are true at the same time about different objects.

```text
grounding  is the record's text still there, still that version   one per claim      needs IO
warrant    what this Anchor's observation has done                one per claim×Anchor  pure
shown      was this claim built from a reading this Anchor took   one per claim×Anchor  pure
depends    does the invariant its author wrote still hold         one per claim      pure
```

They are separate fields, and folding them into a score is precisely the entailment step §1.4 refuses. Each answers a different question and each has a different remedy: a moved ground says re-ask the world, an unseen reading says fix the delivery path, a failed invariant says the author's own condition is gone.

A grounding is absent, not failed, for an uttered claim. There is no document to fetch, so reporting a fetch result would be answering about a file nobody wrote.

The warrant is a pair of enums rather than one, because both halves are routinely true together — a statute observed to change to a new version, and a registry that has been down since. Reporting only the outage discards the one certain thing; reporting only the move claims a currency the system does not have.

```text
holding    Holds · Moved{axes, at} · Incomparable{took, reads} ·
           Absent · NeverEstablished · Undated
knowledge  Seen{at, verifiability} · Blind{since, why}
```

Holding is decided by a diff, not by a sequence comparison. The binding sequence only gates the work: if nothing has been appended since the binding, the answer is settled without reading the log. Past that gate the state as of the binding is folded and compared with the state now, and `axes` names the paths that differ — computed without the substrate knowing what any of those paths mean, which keeps it inside the rule that there is no fixed state vocabulary. A sequence cursor says that the state changed; only the diff says that it changed away from what this claim was bound to, which matters because a recapture advances the cursor while landing on the state it left.

`Incomparable` exists because a reading taken by one instrument and a reading taken by another cannot be diffed into an answer about the world. It is decided after the diff, not before: two instruments producing identical state agree about this target, and answering “incomparable” there would discard positive evidence. When they do differ, a path the newer instrument measures and the older one never did is dropped before the answer is decided, because silence is not disagreement. A path that has *vanished* still counts — an instrument that stopped looking cannot report that what it used to measure is unchanged, and this is what keeps a renamed key from being mistaken for agreement.

Staleness and verifiability are deliberately not variants. How old an observation may be is the caller's threshold; a freshness bound may be handed in as an *instruction* that decides whether to look again before answering, but no verdict about age is returned. Verifiability is a field on a seen observation, because it grades how the observation was obtained and is true simultaneously with whatever the fact did.

Shown is the axis that the citation literature does not have. `Seen` means this Anchor recorded that exact reading at that sequence; `Unseen` means the claim cited a reading this Anchor never took; `NotSaid` means it cited none, which is what a note a person wrote looks like. `Unseen` is the shape of a second computation of the same fact running beside the Anchor instead of through it, and it is why the runtime offers a sampling operation that returns a reading together with its address: the delivery path and the Anchor are then one look at the world rather than two. Shown is kept out of holding on purpose. A fact that changed and an answer assembled somewhere else want opposite responses, and a reader who cannot tell them apart has lost the distinction that made the check worth running.

Depends is the invariant its author wrote down, evaluated over all the Anchors the claim names at once. The expression language gains quantifiers for this rather than a wildcard path, so that inside a quantifier the state root is the Anchor being asked about and every expression that already worked over one Anchor works unchanged over a set.

```text
Holds        the condition its author stated still holds
Broken       it does not
Vacuous      what was written could not have been broken by anything the world can do
Unevaluable  the body did not answer with a yes or a no
Unstated     nobody said anything
```

`Unstated` is a variant rather than a green light, and `Vacuous` refuses a green light one step earlier: an invariant that reads no Anchor state at all is filed apart from one the world could have broken and did not. Whether a channel from the world into the expression exists is decidable from the syntax tree; whether an expression that does read the world is trivially true is not, and the substrate does not pretend to decide it.

An ask may also carry its anchors, citations and invariant inline, for a sentence that has not been recorded anywhere. Such a question is exactly as answerable as one about a stored assertion, and nothing is written — storing every question would turn each one into an assertion nobody reviewed. Which source is used is decided by the data rather than by the caller: a claim that already carries an assertion refuses the inline form, because the alternative is two callers holding different answers about one claim with nothing reporting the disagreement.

Finally, a claim may rest on records that themselves rest on others. A bounded walk over memory links, performed only when the caller asks for it, reports the records reached whose footing is no longer current, together with the path that led to each. It says that a claim reaches something that moved, by this route. It does not say the claim is therefore wrong.

## 5. Why the Anchor is the central abstraction

### 5.1 Direct memory-to-fact attachment is insufficient

A fact payload alone does not contain its provenance or its interpretation. The same JSON payload can be produced by two derivations, and the same derivation can produce different payloads at different times. A memory attached directly to the payload therefore cannot answer whether a new payload is comparable to the old one or whether its difference matters.

The Anchor supplies the missing relation. It names the probe declaration, preserves the rules that interpret the payload, identifies the terminal states, and records the history of decisions made under those criteria.

### 5.2 The Anchor is not a truth authority

The Anchor does not certify that the external fact is true. It certifies only that a particular probe implementation produced a particular result and that a particular set of rules interpreted it in a particular way. Trust remains distributed among the target, the probe author, the transport, the rule author, and the person who accepts a baseline.

This is why the architecture records verifiability without treating it as a truth score. A closed derivation may have a stronger replay story because its output-affecting inputs are content-addressed. It can still observe the wrong target. An open derivation may be useful while depending on an environment that cannot be fully replayed. The distinction must remain visible rather than being collapsed into a misleading boolean such as “trusted”.

### 5.3 The Anchor is a deep module

Depth here is not the count of operations but what each one spares its caller from knowing. The grounding lifecycle is a handful of verbs — open, observe, sample, read, ground, revise, bind, close — around which the runtime also exposes the operational surface a deployment needs: running a scheduled pass, requeuing, reading and writing run settings, resolving the current instrument, walking events since a cursor, and reporting corpus health. None of them asks the caller to assemble journal projection, expression evaluation, version construction, provider comparison, concurrent-append fencing, or append-only storage. That is the leverage, and it gives maintainers locality besides: a change to fencing or address construction is tested and corrected in one implementation rather than in every command that observes an Anchor.

The boundary is not introduced for abstraction's sake. There are multiple real adapters: in-memory and SQLite stores, in-process, script, and shell transports, and Git and Claude Code providers. The seams correspond to actual variation and to public contracts that the runtime must defend.

## 6. Runtime architecture and implementation mapping

### 6.1 The substrate, batteries, and domain

The workspace is split by responsibility rather than by feature names. `gmr-core` contains the vocabulary, newtypes, canonical address functions, observation and entry values, and the pure Journal fold. It does not know how to fetch reality, evaluate expressions, invoke a process, or store a row.

`gmr-expr` is a pure expression engine. It parses paths, literals, operators, time values, `changed`, `exists`, arrays, and objects. It deliberately does not depend on `gmr-core`; the expression evaluator should remain useful without knowing what an Anchor means.

`gmr-probe` defines the invocation seam. Its `Transport` interface resolves a probe name to a derivation and invokes it with a position and budget. Concrete transport implementations are not part of the substrate.

`gmr-content` defines the versioned provider seam for external memory. It knows that content has a provider, an external ID, bytes, and a comparable version; it does not know whether those bytes came from Git, a Claude Code history, or a future provider.

`gmr-store` defines storage interfaces separated by mutability and relation type. Journal history, bindings, sealed blobs, links, settings, and scheduling queue state have different consistency and mutation requirements. They are not combined into one generic repository interface. SQLite is feature-gated, and the testkit provides a reference implementation for conformance tests.

`gmr-runtime` is the only orchestration layer. It combines core values, expressions, transports, content providers, and stores into the operations that make the grounding lifecycle executable. It does not decide what a coding “signature” or “logic” axis means.

`gmr` is a facade that re-exports the substrate and defines nothing of its own.

No crate in the substrate produces a binary. A shipped executable would have to choose a transport, a provider, and a store, and those choices are the domain's; a substrate that made them would stop being domain-neutral no matter how carefully its source avoided domain vocabulary. This is the least obvious way the boundary leaks, because a chosen default reads like configuration rather than like a decision.

The batteries are the reusable implementations no single domain owns and the substrate must not force on anyone: `transport` for the ways a probe can be executed, `provider` for the memory backends, `survey` for language-agnostic walking and fuzzy coordinate matching, and `atlas` for rendering an Anchor-memory graph as a standalone page. That list is open-ended by design — any capability several domains want but that should not grow into the substrate qualifies as another battery.

A storage backend is deliberately not on that list; §12.3 explains why. The coding domain owns language extraction, coordinate matching, shapes, memory frontmatter, and CLI delivery policy under `domains/coding/`.

### 6.2 Runtime services

The runtime is assembled from four capability-focused services. `AnchorLog` wraps only the Journal. `Observer` holds only configured Transports. `MemoryLens` holds bindings, sealed blobs, links, and content providers. `Scheduler` holds the optional Queue, the sighting counters, mutable run settings, and deployment policy. Operations use the smallest service set that can perform their work.

This arrangement is more than an organizational preference. A new operation that only reads Journal history should not accidentally gain access to memory providers or the queue. Capability locality reduces the number of facts that a caller must know and limits the blast radius of future changes.

### 6.3 The coding domain as an assembly

The coding domain wires together the substrate and batteries. It chooses the in-process, script, shell, HTTP, file, and SQL transports, selects the memory providers this build carries, uses survey for fuzzy coordinate matching, and links the language-aware extractor into the CLI as the implementation the in-process transport runs. The substrate is therefore reusable by a different domain without importing a programming-language parser or a particular memory backend.

The repository's own `memories/` directory demonstrates the user-facing side of this assembly. A note's frontmatter names an `about` coordinate, optionally declares a shape and a `watch` set, and the body explains the judgment. The CLI scans these declarations, constructs Anchors, binds references, and later applies delivery policy. The note body is not copied into the Journal.

### 6.4 Host bindings

A host binding is a domain in the same sense the CLI is: it assembles a runtime and translates, and it owns no semantics. The Node binding exposes seven operations — sample, ground, since, bind, revoke, open, close — and a data entrance for declaring probes, so that a caller with no repository and no way to implement a Rust trait can still say what a probe is.

What is left out is the more informative half. Scheduling does not cross: the cadence belongs to whichever process runs the observation loop, and a copy of it in every caller means contending for leases and burning duplicate probe calls. Nothing that changes criteria crosses either, because that is an owner's judgment and belongs in a reviewed commit rather than in product code. The consequence is that a deployment assembling this binding must answer, for itself, which process is doing the observing — the interface does not ask.

The binding also folds nothing, judges nothing, and retries nothing. It deserialises input strictly, so that a misspelled field is refused rather than silently dropped, and it computes hashes for anything a caller sends as text: a declaration's identity must be earned on this side of the boundary or it is not an identity at all.

### 7.1 Opening an Anchor

Opening is not the act of writing a declaration into a table. It is the creation of an observed generation. The runtime first ensures that the key has no history. If the request supersedes another generation, the old generation must already be closed and the supersession rationale is sealed.

The runtime resolves and invokes the probe at the requested initial position. It computes the Observation identity, evaluates the initial rules, and appends an `Open` entry containing the Anchor, the successful Observation, and the resulting state. A malformed initial transition is preserved as a warning and the requested initial state is retained; it is not silently converted into a successful judgment.

Run settings and automatic scheduling are configured after the Journal entry. If operational settings cannot be stored, the Anchor remains open and the runtime reports that it will run under deployment defaults until repaired. This is an operational warning, not a fabricated observation.

### 7.2 Direct observation

Where the deployment has a queue, a direct observation takes the lease before it does anything else, and returns a lease conflict if it cannot. Taking it first is what makes “manual” and “scheduled” the same kind of write rather than two writers who cannot see each other: the honest way to observe by hand is to compete for the token, not to route around it. Where no queue is configured there is no lease to take, and the observation proceeds unfenced.

The runtime then folds the Anchor's Journal. If the projection says the Anchor is closed, the operation returns `Closed` without invoking the probe. Otherwise it resolves the current instrument, obtains the current position from state, invokes the transport under a budget that the Anchor's own setting may narrow but never widen, and constructs an Observation from the returned outcome and derivation.

It then evaluates the transition function with the current state and the two recorded times. A transition result is written with the current observation. A still result may be compacted if the state and fact address are both equal. An evaluator or probe failure is written as an `Attempt`; the last successful observation remains the world reading against which later success will be compared.

The append carries a fencing value when the operation is lease-managed. The operation settles the queue only after the Journal append path has completed.

### 7.3 Scheduled observation

The scheduled path uses a mutable queue because scheduling is operational state, not Anchor semantics. A due ticket grants a lease and increments an epoch. The worker receives the epoch as a fencing token. If it continues after the lease has been replaced, the Journal rejects its stale token.

The guard also refuses an unfenced successful sighting once an Anchor is under lease management. This closes the bypass in which a manual observer could write beside a scheduled worker. Author revisions are deliberately different: they are not observations and remain available through the explicit revision path.

The scheduled pass reschedules successful non-terminal observations according to cadence, and retires an Anchor from the queue once the projected state enters a terminal set. A failure backs off on the streak, except for an unevaluable rule, which goes straight to the policy cap because it will not answer differently later.

A batch spends one shared budget across all of its tickets, so a queue larger than the budget has a tail the clock never reaches. Those tickets are not observed and not blamed: the pass checks the remaining budget before each one and, once it is spent, settles the rest as due immediately without invoking anything. The alternative — invoking them anyway and letting the transport answer instantly with a spent budget — files each one as a failed attempt, which produces a backoff and eventually a stall edge announcing that an Anchor is stuck when the truth is that its turn never came. Reporting a failure that did not happen is the one thing this system exists not to do. Rescheduling at zero rather than at a cadence matters for the same reason: it puts the skipped tail ahead of the anchors this pass did observe, so the next pass resumes where this one stopped instead of starving the tail one cadence at a time while every individual pass looks healthy.

A runtime without a queue remains valid for direct observation; it simply has no automatic lease coordination.

### 7.4 Read, observe, and check

The three operations answer different questions. `read` projects existing history and resolves current memory views without changing the Anchor. `observe` performs a new observation and reports whether the Anchor state machine moved. The coding CLI's `check` performs observation and then combines the result with criteria drift, instrument drift, declaration diagnostics, and memory subscriptions to decide whether a human needs to inspect something.

Two further operations sit beside these and answer the questions of §4.6 rather than §4.5. `sample` reads an Anchor and returns the reading together with its address, so that whoever composes an answer can cite what they were actually shown. `ground` takes claims and reports how they stand. Both accept a freshness bound as an instruction, which decides whether to look again before answering; that is the only way a deployment which never runs a scheduled pass will observe anything at all, and a deployment that reads only the event cursor without either mechanism will correctly and silently report that nothing has changed.

This distinction explains why `observe` and `check` can return different exit codes for the same repository state, and why converging them would be a criteria change rather than a tidying of the command surface. `observe` reports any state-machine transition, on every axis, whether or not a memory is bound to the Anchor at all. `check` reports a delivered memory, an Anchor that moved with nothing bound to it, or a grounding diagnostic. Two consequences follow from the level-triggered rule of §4.5. A movement on an axis that no memory watches is a valid transition but is intentionally quiet at the `check` boundary. Conversely, a run in which nothing moved can still fail `check`, because a subscribed axis set by an earlier observation is still set; that is the intended behaviour, not a spurious repeat.

## 8. Memory protocol

### 8.1 External content and historical versions

The content provider is a temporal interface, not merely a file reader. The current fetch answers what version exists now. Historical fetch answers whether the bytes that existed at binding time can still be recovered. Both are needed to distinguish “the memory was rewritten and we can compare the old and new text” from “the memory was rewritten but its prior version has been lost”.

The runtime represents these possibilities in `MemoryView`. It can expose the bound version, current version, current content, content at bind time, rewrite status, retrievability, and provider unavailability. The memory's body remains outside the Anchor Journal.

### 8.2 Binding as a temporal relation

Binding has two dimensions that must not be merged. The structural dimension is the relation between a memory reference and one or more Anchors. The temporal dimension is the occasion on which that relation was recorded, including the version then in view and, for a single Anchor, its Journal sequence.

For a single-anchor binding, `bound_at_seq` allows a read to determine whether the Anchor has advanced since the binding. For a binding naming several Anchors, there is no one correct sequence number; the runtime therefore leaves this field absent rather than inventing a false comparison point.

Reaffirmation appends a new binding record. It does not erase the older occasion, and it does not alter the Anchor's observation history. This preserves the difference between “the memory was bound at version V1” and “the author later reaffirmed it at version V2”.

### 8.3 Links are not grounding

A link connects one memory reference to another with an application-defined kind, such as elaboration or contradiction. It does not imply that either reference is bound to an Anchor. A bound memory may cause a linked memory to be carried into a read result, but grounding does not propagate across the link.

This is a deliberate refusal to make a universal memory graph semantics. One application may want linked rationale to travel with a decision; another may want a contradiction to suppress retrieval. The Anchor remains the grounding authority, while link traversal remains an auxiliary query policy.

## 9. Historical integrity and change management

### 9.1 Why the Journal is append-only

An ordinary mutable row can tell a reader the current state but not which state was previously accepted, which observation produced it, or which rule version was in force at the time. An append-only Journal turns those questions into queries over history. The current state is a projection, and replay is a validation mechanism rather than an afterthought.

The SQLite implementation enforces append-only semantics with update and delete triggers for Journal, bindings, links, and sealed records. The storage traits are not enough by themselves: a future backend must preserve the same semantic properties and pass the conformance tests.

### 9.2 Criteria are data with history

The transition table, probe declaration, terminal set, and explicitly restated state are criteria. They are not harmless implementation details. A change in any of them can change the interpretation of a future observation and, in the case of a state revision, can change what the current history means to a reader.

The runtime therefore records `Revise` entries with content-addressed context and rationale. The context is what the substrate captured about the Anchor and its current observation. The rationale is what the author supplied. Sealing provides tamper evidence and stable retrieval; it does not turn author prose into an objective fact.

The coding CLI distinguishes accepting a new baseline from accepting a changed declaration. A baseline acceptance recaptures the target and records why the new reading is now accepted. A criteria acceptance changes the probe, rules, or terminal set through an explicit revision. The two judgments cannot silently share one rationale.

### 9.3 Closure and supersession

Closure is irreversible at the generation level. It can be caused by an explicit close action or by entering a terminal state. Once closed, the runtime refuses new observations and criteria revisions and the scheduler retires the Anchor.

If a rule was wrong, the repair is not to reopen the old generation. A new Anchor may explicitly supersede the closed one and seal the reason. Supersession does not migrate bindings, merge Journals, or rewrite the old history. The cost of this separation is that a caller must intentionally establish new bindings; the benefit is that a historical generation cannot acquire a new meaning by accident.

### 9.4 Mutable operational state

Run settings and queue state are intentionally mutable. Cadence, budget, retention mode, lease expiry, due time, and backoff are controls over execution, not changes to what the Anchor judges. Putting them in the immutable criteria Journal would create unnecessary semantic revisions; putting criteria in a mutable settings row would make interpretation drift invisible. The architecture keeps the two forms of change apart.

Sighting counters are mutable for a related but distinct reason. How many times an Anchor has been looked at, and when it was last looked at, is a counter that is overwritten in place. It is not journal material even though it counts observations: the Journal records what happened, one immutable entry per real transition, and appending an entry for an observation that changed nothing would empty that property of its content.

### 9.5 Carrying a store across a storage-schema boundary

The Journal cannot be rebuilt from anything else, so the storage schema's own evolution is a grounding concern rather than a database detail. Two mechanisms cover it, and the split is by direction.

A store stamped older than the running build is carried forward one rung at a time by a migration ladder. A store stamped newer is refused outright, because a shape written by a later generation cannot be known and misreading it is worse than not opening it. The asymmetry is the whole point: refusing both directions would turn every added column into a migration event for every user, while accepting both would let a build guess at a shape it has never seen.

Where the ladder cannot carry a store — a stamp from the future, a shape no rung reaches, or simply a move to another machine — an export and a replaying import are the escape hatch. Four properties make them a contract rather than a convenience.

The export carries the append-only tables only: journal, bindings, their reverse index, links, and sealed records. Run settings and queue state are deliberately left out, on the same grounds as §9.4 — they say how an Anchor is run, not what it judged, and declaration synchronization reconstructs them. Carrying them would import one deployment's operational choices along with another's history.

The export format versions itself independently of the storage schema. The two change for unrelated reasons: a table can gain a column without any exported row changing shape, and a row shape can change without the tables moving. Tying them together would either refuse compatible files or accept incompatible ones.

Rows travel untyped. A Journal entry's body passes through as opaque JSON rather than being parsed into the entry vocabulary. This is what lets a newer binary export a file written by an older one, and that is the direction that matters, because it is exactly where a typed round-trip fails: an entry variant the running build has never heard of has to survive the trip rather than be rejected as unparseable — and surviving upgrades is the reason the file exists.

Import replays only into a store it has proved empty, inside a single transaction, and reasserts that every row landed at the sequence the export recorded. Sequences are referenced across tables, so a row that silently renumbers does not fail — it succeeds while pointing somewhere else. A refusal leaves a store as empty as it started and is recoverable; a quietly rewired history is not.

## 10. Drift taxonomy and reliability analysis

### 10.1 Fact drift

Fact drift occurs when a new successful observation has a different fact address from the previous successful observation. This is the identity-level signal that something about the answer or its derivation changed. It is not, by itself, an explanation. The changed target may have moved, disappeared, or changed shape; the instrument may have changed; or the payload may have changed while the Anchor state remains semantically equal.

The facts, declaration, derivation, evaluator version, and Journal context must be read together to explain the drift.

### 10.2 Instrument drift

Instrument drift occurs when the derivation stored with the latest observation differs from the derivation currently resolved for the Anchor's declared probe. The declaration may be unchanged while the implementation behind the name has been replaced. GMR reports this separately because a new answer under a new instrument is not automatically comparable to an old answer under the previous instrument.

The coding CLI's `instrument_swapped` diagnosis and rebase path are domain-level responses to this condition. The runtime can continue observing, but it does not silently claim that the historical baseline and the current derivation have the same semantics.

### 10.3 Criteria drift

Criteria drift occurs when the live Anchor differs from the declaration that the current domain build says should exist. The difference may be in the probe reference, transition table, or terminal set. The coding sync and check paths report this difference and require an explicit accept or revise operation.

This separation prevents a repository checkout from silently rewriting the meaning of an existing Anchor merely because a declaration file changed. The declaration is a proposal to compare; the Journal is the record of what was accepted.

### 10.4 Evaluator drift

Evaluator drift occurs when the evaluator version stored with an observation differs from the version this build would use to interpret the same rules. The Anchor's declaration is unchanged and the instrument is unchanged, yet a change in comparison, path, or object-construction semantics can move every Anchor in a corpus at once while the world stands still.

This is the third member of the version triple, and the three must not be collapsed. The declaration says what the Anchor asked for and can disagree with reality. The derivation says what actually produced the facts and is known only to whoever executed it. The evaluator version says which semantics interpreted them. Merging any two makes one of them lie: treating the declaration as the derivation is precisely the “the probe changed its logic and its version did not move” failure that earned versions exist to prevent.

Every observation records all three, so a consumer facing a corpus-wide flip can tell a rule upgrade from a world change. The current implementation stops there. It stores the evaluator version but no operation compares it against the running build, so unlike instrument drift there is no diagnosis and no rebase path — the evidence is in the Journal, and reading it is manual. This is a gap in the reporting surface, not in the record.

### 10.5 Memory drift

Memory drift occurs when the provider's current version differs from the version stored in the BindingRecord. It is a standing condition on the memory, not an Anchor transition. The provider may still be able to retrieve the bound version, in which case the caller can compare the two texts, or it may report that the old version is unavailable.

An Anchor remains grounded even if the current memory text changed, because the binding relationship and Anchor history still exist. What changes is the confidence with which a caller can reconstruct what the author originally wrote.

### 10.6 Grounding loss

Grounding loss is the inference-layer counterpart of memory drift, and it is not the same condition. A memory drifts and remains a memory: somebody must read it again and decide. An inference loses its ground, and there is nothing to re-read — the sentence was produced once, from a particular reading, under a condition its author stated.

It has three independent forms, each reported separately. The Anchor's ground moved away from what the claim was bound to. The claim cited a reading the Anchor never took, which means the answer was assembled beside the Anchor rather than through it, and the Anchor was decorative for that claim from the beginning. The invariant the author stated stopped holding. A claim can be in any combination of these, which is why they are not a single field, and none of them is a statement that the sentence is false.

### 10.7 Operational and semantic failure

Probe reachability, timeout, process failure, invalid artifact, invalid output, provider failure, lease conflict, and store failure are not world facts. Rule faults are not world facts either. They are failures of the observation or interpretation path and are represented as attempts or runtime errors according to where they occur.

This distinction controls retry behaviour, and it divides the failures by whether repeating them could produce a different answer. Everything the world might yet answer — an unreachable process, a timeout, an unreadable artifact, output that did not parse — is retried on a streak-driven backoff. An unevaluable rule cannot answer differently on the tenth attempt than on the first, so it goes straight to the policy backoff cap while remaining visible immediately. Repeating a broken rule does not make it meaningful.

### 10.8 What is proved and what is assumed

The architecture proves several properties structurally. Journal replay is deterministic for a fixed sequence. Fact addresses distinguish derivation versions and absence. Successful and failed observation paths are separate. Terminal closure is monotonic in the projection. An append whose stated premise no longer matches the head of the log is refused, on every write path, and stale fencing tokens are refused as well; the guard is a single shared function that every backend calls, so the in-memory reference store and SQLite enforce the same rule. Memory rewrite and Anchor staleness are exposed as distinct fields. A claim's ground, its citation, and its stated invariant are reported as separate answers, so no consumer can read a verdict the substrate did not make.

The architecture assumes several properties it cannot prove from inside the substrate. A transport must honestly earn and report a derivation version. A probe author must include every output-affecting input in that version. A domain must choose a position and rules that cover the failure modes it cares about. A provider must implement version comparison consistently. A human or agent must respond to a surfaced memory rather than ignoring it.

The result is a reliable chain of accountability, not an oracle.

## 11. Failure semantics

### 11.1 Probe failure

When a transport cannot resolve or invoke the requested probe, the runtime appends an `Attempt` with a reason class and failure code. The previous latest Observation and state remain in the projection. A caller can distinguish unreachable, timed out, unusable, invalid artifact, process failure, oversized output, and invalid JSON without inferring from a generic error string.

### 11.2 Expression failure

When a transition guard or new-state expression cannot be evaluated, the runtime does not choose a default state. It appends an `Attempt` carrying a stable code such as missing field, non-object path, non-comparable values, divide by zero, non-boolean guard, or absent new state. The current state and last successful world reading remain unchanged.

### 11.3 Provider failure

When a memory provider cannot fetch current content, the read path returns a MemoryView with an unavailable reason. If the provider says the memory is gone, that is not interpreted as an Anchor transition. If the provider returns a new version but cannot retrieve the bound version, the view preserves the rewrite signal and marks historical retrieval as unavailable.

### 11.4 Concurrency failure

Two orthogonal questions arrive at every append, and they take two answers.

```text
am I still a legal writer?        lease and fencing token   pessimistic, sized for a crash
does what I computed from hold?   the expected head         optimistic, one comparison
```

Correctness rests on the second. A transition entry is a function of the state as of some sequence and an observation, so the append states which sequence it was folded against, and the Journal refuses it if the log has moved on. A worker whose lease expired but whose process is still running cannot land a result computed from a state that is no longer the head; neither can a second writer that never took a lease at all. The runtime's response to a refusal is to fold again and retry, not to fail the caller.

This must cover every write path. A bypass that states no premise — even one added for a single manual invocation — degrades the guarantee to “most of the time”, because the two writers cannot see each other. Author revisions are not exempt: a revision seals an immutable rationale computed from the state its author read, and “the Anchor moved while you were writing the reason” is something to be told rather than something to overwrite in silence. The single exception is an entry whose content is decided by nothing it read — the record of a failed observation, whose attempt count is derived at fold time and never stored — where two concurrent failures each counting once is the correct outcome.

The lease is therefore an efficiency device rather than a correctness one. Its value is that two machines do not fire the same probe and burn the same network call; a writer that cannot take it can leave the observation to the holder. It still requires a monotonic token, so that the Journal can record faithfully under whose scheduling claim an entry was written, but that is a receipt, not a permission slip. Resting correctness on it would silently turn a deployment with no queue into a different system.

### 11.5 Criteria failure

A declaration that differs from an Anchor is not automatically applied. The runtime leaves the existing criteria in place and the domain reports criteria drift. This is a failure of agreement between the live declaration and the accepted generation, not a reason to mutate history in the background.

## 12. Evolution and extension

### 12.1 Adding a probe

A new probe belongs in a domain or reusable battery, behind the `Transport` contract. Its output must distinguish a successful absence from a failure to observe. Its derivation must be earned from the semantic closure of inputs that can change its output. The implementation may run in-process, as a script, or through a shell artifact; transport is an execution mechanism, not the probe's logical identity.

A transport that executes a known operation can also be declared as data rather than in code. A recipe — an HTTP request, a file and a selector, a query and its binds — deserialises into the same declaration type whether it arrived as a configuration file, as JSON handed in by a host binding, or as a literal, so that the version a probe earns does not depend on which door the declaration came through. A recipe carries the name of an environment variable, never its value: it travels over wires, sits in files, and is logged, and a resolved secret would be unrecoverable from all three.

The new probe must not add language knowledge, shell commands, or domain status vocabulary to `gmr-core`. It must provide tests for deterministic output, budget handling, malformed output, absence, and version changes.

### 12.2 Adding a memory provider

A provider must supply a stable provider ID, a stable external identifier space, a comparable current version, and an honest response when an old version cannot be retrieved. The provider must not mutate Anchor state or decide when a memory is grounded. It may report warnings during runtime assembly; those warnings must remain visible rather than disappearing into process startup output.

### 12.3 Adding a storage backend

A storage backend is not a battery. Transports and content providers live outside their contract crates precisely because those contracts are meant to be implemented by anyone; a store's contract is not, because its invariants are not expressible as method signatures. Append-only and fencing are properties of what a backend refuses, and only the crate that defines them can hold the tests and the shared guard that make the refusal real. A backend is therefore a feature and a module inside the storage crate, not a new package. Putting one outside would hand the enforcement of the invariants to the party least able to be checked on them.

Within that crate, a backend implements semantics rather than signatures. Journal and binding history must remain append-only, and the enforcement belongs in the store itself — a trait that merely declines to offer an update method is a convention, not a guarantee. Sealed records must be content addressed and immutable. The Journal must refuse an append whose stated premise no longer matches the head of the log, and must refuse a stale fencing token; the guard is shared rather than reimplemented, so a backend inherits both rules instead of restating them. Current binding queries must return the latest relation without deleting historical occasions. The backend must pass both the in-memory reference contract and the durable contract, and if it participates in export and import it must satisfy §9.5 as well.

### 12.4 Extending the expression language

An expression feature belongs in the pure evaluator if it can be computed from its explicit inputs. It must not add I/O, an ambient clock, randomness, or provider access. If it changes comparison, path, or object-construction semantics, it changes the evaluator version. If it creates a new failure mode, the failure must cross the runtime seam as a structured code.

### 12.5 Adding a domain axis

A state axis belongs to the domain because only the domain knows what it means for that axis to matter to a judgment. The domain must define the probe field, the transition rule that sets the state, the shape that exposes it, the memory subscription that receives it, and the acceptance action that clears or reinterprets it. The substrate should carry the resulting JSON without adding a global vocabulary of “drift” states.

### 12.6 Changing compatibility-sensitive semantics

Changing a probe declaration is criteria drift. Changing the implementation behind a probe name is instrument drift. Changing evaluator semantics is evaluator drift. Changing outcome schema or canonicalization is a Journal and cross-machine compatibility change. Changing rules, terminal states, or the meaning of state revision is a semantic revision. Changing cadence or retention is operational.

The implementation must not disguise one category as another. In particular, an instrument upgrade cannot be silently called a criteria acceptance, and a criteria edit cannot be silently called a refactor.

## 13. Verification strategy

The architecture is validated at the seams where its claims are made. Core tests cover canonical addresses, outcome distinctions, state projection, terminal accumulation, and wire compatibility. Expression tests cover absence, fault propagation, first-match ordering, deterministic evaluation, complete object construction, and time-based rules. Runtime tests cover observation operations, leases, write premises, closure, supersession, revision, memory rewrite, historical retrieval, grounding, citation, invariants, link carry, and event cursors. Store conformance tests run the same behavioural contract against reference and SQLite implementations.

The repository's gate additionally enforces the topology this document describes, because a boundary that only a document defends is undefended. It checks that dependencies point downward across substrate, batteries, and domain; that the pure roots have no workspace dependencies at all; that forbidden I/O and database libraries stay out of the crates that must not reach them; that no substrate crate produces a binary; that the facade defines nothing and still builds with every feature off; that the storage contracts named by this document are the ones the crate actually declares; that a transport says what it observes; that a wire contract's version moves whenever the shape behind it does, and that a hand-written type declaration for a host binding names the version the runtime does; and that the guarded zones contain no comments. These checks are architectural tests: they defend the placement of responsibility rather than a particular implementation line, and each of them corresponds to a decision above rather than to a style preference.

One gate check defends the test suite itself rather than the architecture. The acceptance script must end with a sentinel that prints how many steps ran, and CI must assert that number. This exists because the script was once truncated mid-heredoc; the shell treated the unterminated block as running to end of file, so the script still parsed, still exited zero, and tested almost nothing for two days. Nothing about the code was wrong, and no check that read the code could have noticed. Only a count that two independent files have to agree on catches a suite that silently stopped early.

The most valuable tests are counterexamples to tempting simplifications. They ask what happens when two derivations emit the same facts, when `NotFound` is returned, when a field disappears, when a guard cannot be evaluated, when a memory is rewritten, when an old version cannot be retrieved, when a lease expires while a worker is still running, when two writers fold from the same head, when a claim cites a reading that was never taken, when an invariant is written so that nothing could break it, and when a terminal state is followed by a revision. A future change that makes one of these cases less visible has probably weakened the grounding guarantee even if ordinary happy-path tests still pass.

## 14. Where these responsibilities currently live

This section is a map, not a conformance claim. It says where to look; it does not certify that what is there is right, and a checklist written by the same hand as the design cannot certify that anyway. The architecture above is carried by the current workspace as follows. `crates/gmr-core` contains the Anchor, State, Observation, Outcome, version, binding, Journal entry, canonicalization, and projection vocabulary. The pure transition language is implemented by `crates/gmr-expr`; runtime translation from an Anchor's rules to that evaluator is in `crates/gmr-runtime`.

Probe invocation is defined by `crates/gmr-probe` and assembled by the runtime's Observer. Concrete transports are supplied by `batteries/transport`. The coding extractor and its earned versions are in `domains/coding/extract`. Memory fetching is defined by `crates/gmr-content`, implemented for the repository's providers under `batteries/provider`, and assembled through `MemoryLens`. Language-agnostic walking and fuzzy coordinate matching are in `batteries/survey`, and `batteries/atlas` renders an Anchor-memory graph as a standalone page.

Persistence interfaces, the SQLite implementation, and the portable export and import of §9.5 are in `crates/gmr-store`. The Journal, BindingStore, Sealer, LinkStore, Settings, Sightings, and Queue are separate contracts because their mutability and concurrency semantics differ; `Chained`, which reports where an append-only chain was broken, is a capability a backend may decline to implement rather than a contract every store owes. The budget vocabulary that every outbound call shares is in `crates/gmr-budget`, which depends on nothing. Grounding — the warrant, the citation check, the invariant, and the bounded link walk of §4.6 — is in `crates/gmr-runtime`; `crates/gmr` provides the public re-export facade; the coding CLI performs declaration synchronization and memory delivery, and `domains/node` carries the host binding of §6.4.

This mapping is evidence for the design, not a replacement for the design. A file may move while the responsibility remains. Conversely, moving a responsibility across these seams is an architectural change even if the public command names remain the same.

## 15. Conclusion

GMR is best understood as a temporal grounding system for judgments. It does not attempt to make memory autonomous, facts universal, or probes infallible. It establishes a disciplined relationship among four things that otherwise drift apart: what was observed, how it was derived, how the observation was interpreted, and which human or agent judgment depended on that interpretation.

Two kinds of judgment lean on that relationship. A reviewed constraint that must survive the code changing under it comes back to a person when it does. A conclusion reached in one sitting carries the reading it was built from and the condition its author said kept it standing, and can be asked, later and by somebody else, whether either still holds.

The Anchor is the centre of that relationship. The Probe supplies an answer but does not decide its meaning. The Observation preserves provenance but does not rewrite memory. The transition function gives a domain a small, deterministic language for interpreting change. The Journal preserves the decisions that made the current state possible. The provider preserves memory content and its version history without turning memory into a duplicate fact store. The runtime surfaces a judgment when the declared semantics make it relevant, but leaves the final decision to its caller.

That division of responsibility is the mechanism by which GMR reduces hallucination. It does not make every retrieved statement true. It makes the conditions under which the statement should be trusted, questioned, or reconsidered explicit enough to observe, version, replay, and audit.

## Appendix A. Repository bootstrap example

The coding domain uses the repository itself as a GMR client. A memory note in `memories/` declares a coordinate in frontmatter and explains the judgment in prose. The coding CLI routes that coordinate to a domain probe, derives a transition table from a named shape or from explicit rules, opens or aligns an Anchor, and binds the external memory reference to it. Subsequent observations run through the runtime and are stored in the local SQLite state.

The coding domain will also read declarations from an optional `.anchor/anchors.toml`, but this repository deliberately has none and initialization does not create one. A bare key in that file binds without declaring, which puts the declaration in one place and the memory in another; when a note's frontmatter carries the whole declaration, the two live in the same file and the TOML has nothing left to say. Its remaining uses are the narrow set of cases a coordinate cannot express — hand-written rules, a probe whose field of view is not the repository root, a position key with no coordinate syntax — and every anchor whose memory is *not* a file in this repository: a record kept in an agent's own store or behind a declared provider is bound by address, and the declaration has to live somewhere the repository can carry. The domain's own linter reports a long-hand declaration that a coordinate could have routed. `.anchor/probes.toml` describes probe recipes and `.anchor/providers.toml` declares memory stores this binary was not compiled with; both are tracked, while observation state and artifacts under `.anchor/` are not. Neither file changes the substrate's ontology. `architecture.toml` configures the repository's dependency gate and is not part of GMR's runtime data.

This arrangement lets the repository supervise its own architecture. The memories are user records, the probe recipes and shapes are domain policy, and the crates are the reusable anchoring machinery. Keeping those categories separate is itself one of the relationships that the system is designed to protect.

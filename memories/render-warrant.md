---
about:
  - domains/coding/cli/src/render.rs#holding
  - domains/coding/cli/src/render.rs#knowledge
  - domains/coding/cli/src/render.rs#warranting
  - domains/coding/cli/src/render.rs#unseen
watch: [sig, logic]
---

# A classification nobody prints was never made

`Warrant` ([[runtime-warrant]]) is a closed answer on two axes, and for a while
it reached exactly one surface: `gmr atlas`, through two `if let` arms that
matched `Moved` and the `Blind { .. }` wildcard. Everything else — which of the
three failure classes, whether the probe's closure was open, `Incomparable`,
`Undated` — was computed, stored, serialised into `--json`, and then said to
nobody.

That is the failure mode `probe-Verifiability.md` names for its own enum: the
whole reason it exists is to force a sentence to be said. A grade that is only
ever a field is not being said.

## The three classes are three different people's problem

`Blind` splits `Unreachable` / `Unusable` / `Unevaluable` because
[[journal-FailureCode]] keeps them apart, and the split only earns its keep if
the sentences do too — a store that will not answer, a probe whose output cannot
be used, and rules that cannot be evaluated go to three different people. So
each line names whose it is rather than saying "the last look failed" three
ways.

`NeverAsked` reads as a budget or a first run, never as somebody's fault: it is
our clock running out, and [[content-budget]] makes the same split on the other
side.

An open closure is printed the same way, and now names its surface rather than
gesturing at one: "something outside its version can change the answer" was true
of every open probe and told a reader nothing they could act on. `Verifiability`
carries `over` ([[probe-Verifiability]]), so the line says *the interpreter that
runs it* or *a remote system* — and `Unknown` prints as "something nobody
recorded", which is the one case where the vague sentence is the honest one.

## One phrasing, two surfaces

`gmr read` prints these as trailing clauses and `gmr atlas` hangs them off nodes
as facts. Both call the same two functions, which return `Option<String>` and
answer `None` when there is nothing to say — the shape `grounding` next to them
already had. Two renderings of one enum drift; the whole point of a closed
answer is that every surface says the same thing about the same value.

`Holds` and a `Closed` reading print nothing at all, for the reason
`Grounding::Current` prints nothing: a line per memory saying everything is fine
is a line nobody reads, and it buries the ones that are not.

## When this changes, ask

Does a `Holding` or `Blind` variant arrive without a sentence here? Then it is
back to being classified and not reported, and the compiler will say so — both
matches are exhaustive, which is the only reason this stays true.

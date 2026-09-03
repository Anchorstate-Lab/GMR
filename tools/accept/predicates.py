"""The closed vocabulary every scenario is allowed to assert in.

Six predicates, and not one of them matches prose. That is deliberate and it is
the reason this gate can survive a decade of rewording: the sentences in
`render.rs` change every month, the promise does not. A gate whose assertions
are `grep` on human output teaches its maintainers to soften assertions whenever
they change a sentence, and a softened assertion is how a gate dies green.

Wording still deserves checking. It does not deserve a veto, so it lives in a
separate advisory pass and never turns this gate red.
"""


class Broken(AssertionError):
    """A promise did not hold. Carries which one, so the report can group."""

    def __init__(self, guarantee, message):
        self.guarantee = guarantee
        super().__init__(f"[{guarantee}] {message}")


# ── G1  取得出 ──────────────────────────────────────────────────────────────


def _shown(one):
    for anchored in one.get("on", []):
        if anchored.get("anchored") == "on":
            return (anchored.get("evidence") or {}).get("shown")
    return None


def _named(one):
    claim = one.get("claim") or {}
    return claim.get("said")


def a_conclusion_stands(gmr, res):
    """Nothing it named has moved, and it looked where it says it looked."""
    body = res.body or []
    broken = [_named(one) for one in body if one.get("depends") == "broken"]
    unseen = [_named(one) for one in body if _shown(one) == "unseen"]
    if broken or unseen:
        raise Broken(
            "G1",
            f"a conclusion made against the reading in front of it does not stand: "
            f"broken={broken} unseen={unseen}; exit {res.code}",
        )
    if res.code != 0:
        raise Broken("G1", f"nothing is wrong and the exit code is {res.code}")


def a_conclusion_no_longer_stands(gmr, res):
    """What it said it rested on moved, so it comes back — and the exit says so.

    This is the inference loop, and it is not the memory loop: a memory that
    drifts is handed back for a person to re-read, while a conclusion whose own
    stated condition failed needs nobody to adjudicate it.
    """
    body = res.body or []
    if res.code == 0:
        raise Broken(
            "G1",
            "the ground under a conclusion moved and nothing came back. An author who "
            "stated an invariant is the authority on whether it survives; one who stated "
            "none has said nothing, and saying nothing must not buy a green light: "
            f"{[(_named(o), o.get('depends')) for o in body]}",
        )
    if res.code != 1:
        raise Broken(
            "G1",
            f"a conclusion no longer stands and the exit code is {res.code}, so nothing "
            "driving this from a script would ever find out",
        )


def told_at_the_door_it_supervises_nothing(gmr, res, key):
    """A binding onto a key nothing opened is recorded, and the door says so.

    The record layer stays judgment-free -- a binding is a declaration, and a
    deployment may legitimately declare before it opens. What it must not do is
    stay silent at the one moment the writer is present to hear it: a typo'd
    key otherwise supervises nothing until a later doctor run finds it.
    """
    body = res.body or {}
    unopened = set(body.get("unopened") or [])
    if key not in unopened:
        raise Broken(
            "G3",
            f"a binding landed on `{key}`, which nothing ever opened, and the door said "
            f"nothing -- silence published as a supervised look that will never happen: {body}",
        )


def told_apart_by_what_they_looked_at(gmr, res, seen, unseen, silent):
    """Three conclusions, three answers: looked, looked elsewhere, did not say.

    Collapsing the last two is what makes an anchor decorative -- a claim that
    cited nothing and a claim that cited a reading nobody took are not the same
    defect, and only one of them is a defect at all.
    """
    got = {"seen": set(), "unseen": set(), "not_said": set()}
    for one in res.body or []:
        mark = _shown(one)
        if mark in got:
            got[mark].add(_named(one))
    want = {"seen": set(seen), "unseen": set(unseen), "not_said": set(silent)}
    if got != want:
        raise Broken(
            "G1",
            f"what each conclusion was looking at is not told apart: {got} != {want}",
        )


def nothing_is_still_being_asked_about(gmr, res):
    """Retiring one stops it being asked about, without deleting what it said."""
    body = res.body or []
    if body:
        raise Broken(
            "G1",
            f"a retired conclusion is still being asked about: {[_named(o) for o in body]}",
        )


def handed_back_exactly(gmr, res, expected):
    """The set handed back equals `expected` — not a superset.

    Containment is not enough and never was. An agent handed forty memories on a
    one-line change learns to discount every one of them, and a runtime whose
    output is routinely discarded has negative value: it spent the reader's
    attention to make the reader stop reading. Precision is a promise, so it is
    asserted as equality.
    """
    got = gmr.handed_back(res)
    want = set(expected)
    if got != want:
        raise Broken(
            "G1",
            f"handed back {sorted(got)}, expected exactly {sorted(want)}"
            + (f"; extra {sorted(got - want)}" if got - want else "")
            + (f"; missing {sorted(want - got)}" if want - got else ""),
        )


def content_reaches(gmr, key, address, marker):
    """The memory's own bytes came back, and they are the bytes that were written.

    The promise is that the memory is handed over, not that its name is. A run
    that returns an address and no content has handed the reader a filename and
    called it a memory.
    """
    body = gmr.content_of(key, address)
    if body is None:
        raise Broken(
            "G1",
            f"{address} on {key} came back with no content — an address is not a memory",
        )
    if marker not in body:
        raise Broken(
            "G1", f"{address} on {key} came back with content that is not what was written"
        )


def address_roundtrips(gmr, address):
    """An address this runtime printed is an address this runtime takes back.

    Two files edited by different people have to agree on one string. Nothing
    but a round trip can tell whether they still do.
    """
    if gmr.reaffirm(address).code != 0:
        raise Broken("G1", f"`reaffirm` refused the address the runtime printed: {address}")
    if gmr.cobound(address).code != 0:
        raise Broken("G1", f"`cobound` refused the address the runtime printed: {address}")


# ── G2  迁移等价 ────────────────────────────────────────────────────────────


def migrates_equivalently(before, after):
    """Everything the promise rests on survived export → import into a new instance.

    A runtime instance is not a git clone and does not pretend to be. Migration
    is the only channel by which memories cross instances, which makes it the
    lifeline of the whole promise rather than a corner verb.
    """
    for field in ("signals", "memories", "pending", "provenance"):
        if before[field] != after[field]:
            raise Broken(
                "G2",
                f"{field} did not survive migration: "
                f"before {sorted(before[field])} / after {sorted(after[field])}",
            )


# ── G3  该安静时安静 ────────────────────────────────────────────────────────


def silent(gmr, res, why):
    """Nothing moved, so nothing was handed over, and the exit code says so."""
    handed = gmr.handed_back(res)
    if handed or res.code != 0:
        raise Broken(
            "G3",
            f"{why}: expected quiet, got exit {res.code} handing back {sorted(handed)}",
        )


def loud(res, why, bucket=None):
    """Something the reader must see happened, and it was reported as a category.

    The category is read out of `--json`, never out of a sentence. A refusal that
    only exists as prose cannot be acted on by the agent this runtime serves.
    """
    if res.code == 0:
        raise Broken("G3", f"{why}: the run exited 0 — silence was published as agreement")
    if bucket is not None:
        rows = (res.body or {}).get(bucket)
        if not rows:
            raise Broken("G3", f"{why}: nothing was reported under `{bucket}`")


# ── G4  可追溯到真实信号源 ──────────────────────────────────────────────────


PROVENANCE = ("signal", "sources", "asserted_at", "reading_then", "instrument")


def provenance_complete(gmr, key, address):
    """From a memory, the chain back to a real signal source is readable.

    Which signal, who asserted the link, when, what the signal read at the time,
    and which instrument took that reading. This is the product: an argument an
    agent makes must be traceable to something that actually exists.
    """
    p = gmr.provenance_of(key, address)
    if p is None:
        raise Broken("G4a", f"{address} is not reachable from {key} at all")
    missing = [f for f in PROVENANCE if not p.get(f)]
    if missing:
        raise Broken("G4a", f"{address} on {key} cannot be traced back: no {', '.join(missing)}")


def grounding_is(gmr, key, address, kind):
    """How the record stands against its store, as a category rather than a sentence."""
    g = gmr.grounding_of(key, address)
    got = (g or {}).get("grounding")
    if got != kind:
        raise Broken("G4b", f"{address} on {key} grounds as `{got}`, expected `{kind}`")


def reported(res, bucket, containing, why):
    """A `doctor` bucket names this address or key. Buckets are typed; prose is not."""
    rows = (res.body or {}).get(bucket)
    if rows is None:
        raise Broken("G4b", f"{why}: `doctor --json` has no `{bucket}` at all")
    flat = {r if isinstance(r, str) else r.get("anchor") or r.get("note") for r in rows}
    if containing not in flat:
        raise Broken("G4b", f"{why}: `{bucket}` is {sorted(x for x in flat if x)}, missing {containing}")


def not_reported(res, bucket, containing, why):
    rows = (res.body or {}).get(bucket) or []
    flat = {r if isinstance(r, str) else r.get("anchor") or r.get("note") for r in rows}
    if containing in flat:
        raise Broken("G4b", f"{why}: `{bucket}` should not name {containing}")


# ── G5  变化可辨 ────────────────────────────────────────────────────────────


def distinguishable(prints, why):
    """Different things happening to a signal leave different reports.

    GMR does not decide what a change means — the reader does, by recapturing
    the anchor or by rewriting the memory. That division only works if the
    report says *which* change happened. If a renamed signal, a relocated one
    and a deleted one all read the same, the reader has been told something
    moved and given nothing to act on, and the runtime has handed its own
    judgment problem back as a guess.
    """
    seen = {}
    for name, fp in prints.items():
        k = tuple(sorted(fp.items()))
        seen.setdefault(k, []).append(name)
    collisions = [names for names in seen.values() if len(names) > 1]
    if collisions:
        detail = "; ".join(" and ".join(sorted(c)) for c in collisions)
        raise Broken("G5", f"{why}: indistinguishable reports for {detail}")


# ── G6  runtime 的「一定」 ──────────────────────────────────────────────────


def converges(rounds, why):
    """Every due signal is reached in finite rounds, however tight the budget.

    A runtime promises that a change is noticed, not that it is noticed this
    instant. The operational form of that promise is convergence: the backlog
    falls and reaches zero, and no signal is starved while others are served.
    Without it, `a change is always handed back` degrades into `handed back if
    the budget happened to reach it`, which is not a promise at all.
    """
    backlog = [r["skipped"] for r in rounds]
    if backlog[-1] != 0:
        raise Broken(
            "G6", f"{why}: after {len(rounds)} rounds the backlog is still {backlog[-1]} ({backlog})"
        )
    if any(b - a > 0 for a, b in zip(backlog, backlog[1:])):
        raise Broken("G6", f"{why}: the backlog grew instead of draining ({backlog})")


def starves_nobody(seen, expected, why):
    if set(seen) != set(expected):
        missing = sorted(set(expected) - set(seen))
        raise Broken("G6", f"{why}: never observed at all across every round: {missing}")

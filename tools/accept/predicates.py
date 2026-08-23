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

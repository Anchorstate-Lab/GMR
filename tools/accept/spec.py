"""Layer 0 — what GMR promises, written so that no domain appears in it.

Read this file as the product. Every scenario names something that happens in
the world and what the runtime owes the reader when it does; none of them names
a function, a signature or a parse tree, because those belong to the one domain
that ships today rather than to the promise.

The promise, in one sentence: **an argument an agent makes must be traceable to
a signal that really exists, and when that signal moves the memory resting on it
must come back to be judged again.**

    G1  取得出      the memory itself comes back, exactly the right ones
    G2  迁移等价    it survives crossing from one runtime instance to another
    G3  出声/安静   noise is not a change, and silence is never published as agreement
    G4a 可追溯      from a memory, the chain back to its signal is readable
    G4b 可重读      the signal can be read again, and a changed instrument shows
    G5  变化可辨    different changes leave different reports, so a reader can act
    G6  收敛        a tight budget delays a reading; it never loses one

Adding a scenario here is adding a promise. Deleting one is retracting a promise
and belongs to the owner, not to whoever is making the suite green today.
"""

from . import predicates as p

SCENARIOS = []


def scenario(guarantee, question, varies=("world", "store"), needs=()):
    def take(fn):
        SCENARIOS.append(
            {
                "id": fn.__name__.replace("_", "-"),
                "guarantee": guarantee,
                "question": question,
                "varies": tuple(varies),
                "needs": tuple(needs),
                "run": fn,
            }
        )
        return fn

    return take


# ── G1 取得出 ───────────────────────────────────────────────────────────────


@scenario("G1", "the signal moved: does the memory itself come back?")
def the_memory_comes_back_when_its_signal_moves(c):
    address, text = c.put("why.md")
    c.bind(address)
    c.settle()

    c.happen("reading_changed")
    res = c.gmr.check()

    p.handed_back_exactly(c.gmr, res, {address})
    p.content_reaches(c.gmr, c.world.signal, address, text)
    p.address_roundtrips(c.gmr, address)


@scenario("G1", "does it hand back only the memories that are about what moved?")
def only_the_memories_about_what_moved_come_back(c):
    mine, _ = c.put("mine.md")
    c.bind(mine)
    other_signal = c.world.declare_many(c.gmr, c.repo, 1)[0]
    theirs, _ = c.put("theirs.md")
    c.bind(theirs, signals=[other_signal])
    loose, _ = c.put("loose.md")
    c.settle()

    c.happen("reading_changed")
    res = c.gmr.check()

    p.handed_back_exactly(c.gmr, res, {mine})


@scenario("G1", "an agent binds what it just wrote, before any store can answer")
def a_memory_bound_before_the_store_can_answer_still_arrives(c):
    address = c.store.address("fresh.md")
    c.gmr.attest(address, [c.world.signal])
    c.settle()

    written, text = c.put("fresh.md")
    c.gmr.attest(written, [c.world.signal])
    c.happen("reading_changed")
    res = c.gmr.check()

    p.handed_back_exactly(c.gmr, res, {address})
    p.content_reaches(c.gmr, c.world.signal, address, text)


# ── G2 迁移等价 ─────────────────────────────────────────────────────────────


@scenario("G2", "does the promise survive crossing into a new runtime instance?")
def migration_carries_the_whole_promise(c):
    address, text = c.put("why.md")
    c.bind(address)
    c.settle()
    c.happen("reading_changed")
    c.gmr.check()

    before = c.summary()
    fresh = c.migrate()
    after = fresh.summary()

    p.migrates_equivalently(before, after)
    p.content_reaches(fresh.gmr, fresh.world.signal, address, text)


@scenario("G2", "does replaying into a live instance get refused rather than merged?")
def migration_refuses_to_merge_into_a_live_instance(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()
    dump = c.root / "carry.jsonl"
    c.gmr.export(dump)

    res = c.gmr.import_(dump)

    p.loud(res, "importing over a store that already holds history")


# ── G3 出声 / 安静 ──────────────────────────────────────────────────────────


@scenario("G3", "the representation changed but the fact did not: does it stay quiet?")
def noise_is_not_a_change(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    c.happen("noise")
    res = c.gmr.check()

    p.silent(c.gmr, res, "the representation moved and the fact did not")


@scenario("G3", "nothing happened at all: does it stay quiet?")
def a_world_that_did_not_move_hands_back_nothing(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    res = c.gmr.check()

    p.silent(c.gmr, res, "nothing in the world moved")


@scenario(
    "G3",
    "a store that will not answer: is that different from a record that is gone?",
    needs=("store_can_vanish",),
)
def a_silent_store_is_not_a_missing_record(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    blind = c.without_store()
    res = blind.doctor()

    p.reported(res, "unreachable", address, "the store could not be reached")
    p.not_reported(res, "gone", address, "a store that would not answer is not a deleted record")
    if res.code != 0:
        raise p.Broken(
            "G3",
            "somebody else's store being unreachable turned the gate red; "
            "nobody holding this repository can act on that",
        )


@scenario("G3", "the store says the record is gone: is the reader told?")
def a_record_the_store_says_is_gone_is_reported(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    c.drop("why.md")
    res = c.gmr.doctor()

    p.reported(res, "gone", address, "the store says this record is gone")
    p.loud(res, "a binding pointing at a record that no longer exists")


@scenario(
    "G3",
    "a budget too small to look: does it refuse, and does refusing become state?",
    varies=("world",),
)
def a_spent_budget_refuses_and_never_becomes_state(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    starved = c.gmr.check(budget_ms=1)
    p.loud(starved, "the budget ran out before anything could be looked at")

    p.silent(c.gmr, c.gmr.check(), "the same signal, given time to be looked at")


# ── G4 可追溯 ───────────────────────────────────────────────────────────────


@scenario("G4a", "from a memory, can the chain back to a real signal be read?")
def a_memory_can_be_traced_back_to_its_signal(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    p.provenance_complete(c.gmr, c.world.signal, address)


@scenario("G4b", "the memory was rewritten under the binding: does it still read as current?")
def a_rewritten_memory_does_not_read_as_current(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()
    p.grounding_is(c.gmr, c.world.signal, address, "current")

    c.rewrite("why.md")

    p.grounding_is(c.gmr, c.world.signal, address, "rewritten")


@scenario("G4b", "after a judgment is sealed, does the signal go quiet and stay judgeable?")
def a_sealed_judgment_quiets_the_signal_and_a_later_move_wakes_it(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    c.happen("reading_changed")
    p.loud(c.gmr.check(), "the signal moved")

    c.gmr.adjudicate(c.world.signal, "looked at it; the memory still holds")
    p.silent(c.gmr, c.gmr.check(), "the judgment was sealed")

    c.happen("ceased")
    p.loud(c.gmr.check(), "the signal moved again after a judgment")


@scenario("G4b", "judgment after a revert: is what gets pinned the world as it is now?")
def a_judgment_pins_a_fresh_look_not_the_last_reading(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    c.happen("reading_changed")
    p.loud(c.gmr.check(), "the signal moved")
    c.revert()

    c.gmr.adjudicate(c.world.signal, "looked; the change was taken back")

    p.silent(c.gmr, c.gmr.check(), "the world was put back and then judged")


# ── G5 变化可辨 ─────────────────────────────────────────────────────────────


@scenario(
    "G5",
    "can a reader tell which change happened, and so what to do about it?",
    varies=("world",),
)
def different_changes_leave_different_reports(c):
    prints = {}
    for event in ("still", "reading_changed", "ceased", "identity_changed", "location_changed"):
        if event not in ("still",) and not c.world.can(event):
            continue
        with c.sibling() as twin:
            address, _ = twin.put("why.md")
            twin.bind(address)
            twin.settle()
            if event != "still":
                twin.happen(event)
            prints[event] = twin.gmr.fingerprint(twin.world.signal, address)

    p.distinguishable(prints, "the same report for two different changes")


# ── G6 收敛 ─────────────────────────────────────────────────────────────────


@scenario(
    "G6",
    "under a budget too tight for one round: is every signal still reached?",
    varies=("world",),
)
def every_due_signal_is_reached_in_finite_rounds(c):
    keys = set(c.world.declare_many(c.gmr, c.repo, 24))
    c.settle()

    rounds, seen = [], set()
    for _ in range(40):
        res = c.gmr.sweep(budget_ms=40)
        body = res.body or {}
        rounds.append({"skipped": body.get("skipped", 0), "observed": body.get("observed", 0)})
        seen.update(c.gmr.seen_keys())
        if rounds[-1]["skipped"] == 0 and len(rounds) > 2:
            break

    p.converges(rounds, "a backlog under a tight budget")
    p.starves_nobody(seen & keys, keys, "signals waiting behind a tight budget")

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


@scenario(
    "G1",
    "a conclusion says what it rested on: does it come back when that moves?",
    needs=("invariant",),
)
def a_conclusion_comes_back_when_the_ground_it_named_moves(c):
    c.settle()
    seen = c.reading()
    c.gmr.said(
        "what this run concluded",
        on=[c.world.signal],
        saw=[seen],
        depends=c.world.invariant,
        ident="one",
    )
    p.a_conclusion_stands(c.gmr, c.gmr.standing())

    c.happen("reading_changed")
    c.gmr.observe()
    p.a_conclusion_no_longer_stands(c.gmr, c.gmr.standing())


@scenario("G1", "a conclusion nobody looked before making: is it told apart from one that did?")
def a_conclusion_built_beside_the_anchor_is_not_mistaken_for_one_built_through_it(c):
    c.settle()
    seen = c.reading()
    c.gmr.said("looked first", on=[c.world.signal], saw=[seen], ident="looked")
    c.gmr.said("guessed", on=[c.world.signal], saw=["a" * 64], ident="guessed")
    c.gmr.said("cited nothing", on=[c.world.signal], ident="bare")

    p.told_apart_by_what_they_looked_at(
        c.gmr, c.gmr.standing(), seen={"looked"}, unseen={"guessed"}, silent={"bare"}
    )


@scenario("G1", "a conclusion retired: does it stop being asked about?")
def a_retired_conclusion_stops_being_asked_about(c):
    c.settle()
    c.gmr.said("a finding that has served its purpose", on=[c.world.signal], ident="done")
    p.a_conclusion_stands(c.gmr, c.gmr.standing())
    c.gmr.standing("said:done", retire=True)
    p.nothing_is_still_being_asked_about(c.gmr, c.gmr.standing())


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


@scenario(
    "G1",
    "a memory that asked about one axis: does another axis moving leave it alone?",
    needs=("per_note_watch", "has_axes"),
)
def a_memory_that_asked_about_another_axis_stays_put(c):
    watchful, _ = c.put("watchful.md")
    c.bind(watchful)
    narrow = c.subscribe("narrow.md", axes=["surface"])
    c.settle()

    c.happen("reading_changed")
    res = c.gmr.check()

    p.handed_back_exactly(c.gmr, res, {watchful})

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


@scenario("G1", "the front door bound a record: does its own report say it did?")
def an_act_of_grounding_is_reported_as_having_happened(c):
    address, _ = c.put("why.md")

    res = c.gmr.raw("anchor", c.world.signal, "--record", address, json_out=True)

    if res.body is None:
        raise p.Broken(
            "G1",
            "the front door's report could not be read as one answer. An agent that "
            "cannot parse it cannot tell a grounding that happened from one that did "
            "not, and the safe reading — believing the memory anyway — is the one thing "
            "this tool exists to prevent",
        )
    said = res.body.get("bound")
    if not said or said.get("record") != address:
        raise p.Broken(
            "G1",
            f"a record was bound and the report said {said!r}. A report that denies an "
            "act it just performed sends the reader back to trusting a memory nothing "
            "vouches for",
        )
    if res.body.get("barren") is not False:
        raise p.Broken(
            "G1",
            "the anchor was reported as owing a memory in the same answer that says one "
            "was just bound to it",
        )


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


@scenario(
    "G3",
    "an address naming a store this run cannot resolve: refused, or written down against another?",
    varies=("world",),
)
def a_store_this_run_cannot_name_is_never_another_stores_record(c):
    stranger = "nowhere:why.md"

    res = c.gmr.raw("anchor", c.world.signal, "--record", stranger, check=False)

    if res.code == 0:
        raise p.Broken(
            "G3",
            "an address naming a store nothing here registered was accepted. Failing to "
            "resolve a store is our failure; recording it against whichever store happens "
            "to be the default turns it into that store's answer, and `gone` is a state "
            "no reader can tell from a record somebody really deleted",
        )

    after = c.gmr.doctor()
    p.not_reported(
        after,
        "gone",
        "nowhere",
        "a store we could not name became a record another store reports as deleted, "
        "and the binding table only ever grows",
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
    # A budget has to be out of reach by construction, not by being small enough
    # on a fast machine. A margin thinner than the runner's own noise goes
    # flaky, and a flaky assertion is worse than no assertion: it teaches
    # whoever meets it to re-run until green, and then to delete it.
    c.world.declare_many(c.gmr, c.repo, 150)
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


@scenario("G4b", "is the instrument named by what it is, rather than by its bytes?", varies=())
def an_instrument_is_named_by_what_it_is(c):
    listed = c.gmr.raw("probes", "list", json_out=True).body or {}
    unearned = [
        row.get("probe")
        for row in listed.get("probes", [])
        if len(row.get("version") or "") != 64
    ]
    if unearned:
        raise p.Broken(
            "G4b",
            "an instrument whose identity is not a hash over everything that can "
            f"change its answer cannot be compared across machines: {unearned}",
        )


@scenario(
    "G4b",
    "the instrument was swapped: is a reading it did not take reported as such?",
    varies=("world",),
    needs=("swappable_instrument",),
)
def a_swapped_instrument_is_not_mistaken_for_a_moved_signal(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    c.world.swap_instrument(c.repo)
    res = c.gmr.check()

    p.loud(res, "the reading on file was taken by an instrument this build no longer has")
    p.reported(res, "instrument_swapped", c.world.signal, "a swapped instrument")

    c.gmr.recapture("the instrument changed; recapture against the one this build has", every=True)
    p.silent(c.gmr, c.gmr.check(), "the reading was recaptured with the instrument at hand")


@scenario(
    "G7",
    "the signal is gone: can the reading say so, or does a shared category keep it alive?",
    varies=("world",),
    needs=("matched_by_coordinate",),
)
def a_signal_that_is_gone_can_say_it_is_gone(c):
    key = c.world.declare_classified(c.gmr, c.repo)
    c.gmr.check(key)

    c.happen("ceased")
    c.gmr.check(key)
    facts = c.gmr.facts_of(key)

    if facts.get("found") is not False:
        raise p.Broken(
            "G7",
            "the signal is gone and the reading still says found — it matched only "
            f"{facts.get('matched')} and answered with `{(facts.get('at') or {}).get('name')}`. "
            "A category a signal belongs to is not evidence that the signal is there, "
            "and while one classifier can keep a dead coordinate alive, `not there any "
            "more` is an answer the world can never give",
        )


@scenario(
    "G7",
    "the signal is gone: does the anchor quietly start standing on a neighbour instead?",
    varies=("world",),
    needs=("has_neighbour",),
)
def an_anchor_never_silently_takes_up_a_different_object(c):
    neighbour = c.world.neighbour()
    c.gmr.declare(neighbour)
    c.settle()
    mine = c.gmr.facts_of(c.world.signal).get("facts")
    theirs = c.gmr.facts_of(neighbour).get("facts")
    if mine == theirs:
        raise RuntimeError("the fixture's two signals already read alike; it proves nothing")

    c.happen("ceased")
    c.gmr.check()
    now = c.gmr.facts_of(c.world.signal).get("facts")
    # The neighbour has to be read again, not remembered: it may have shifted for
    # its own reasons, and a stale snapshot would let the two look different while
    # they are in fact the very same reading.
    theirs = c.gmr.facts_of(neighbour).get("facts")

    if now == theirs:
        raise p.Broken(
            "G7",
            f"`{c.world.signal}` is gone, and the reading it now stands on is "
            f"`{neighbour}`'s — reported as this same object having an attribute "
            "change. Two anchors now rest on one object, every later reading is "
            "compared against the wrong one, and the memory bound here is judged "
            "against code it was never about",
        )


# ── G8 义务持久 ─────────────────────────────────────────────────────────────


@scenario(
    "G8",
    "the signal moved and nobody has judged it: does the next run still say so?",
    varies=("world",),
)
def an_outstanding_judgement_is_announced_until_it_is_answered(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    c.happen("reading_changed")
    p.loud(c.gmr.check(), "the signal moved")

    again = c.gmr.check()
    if address not in c.gmr.handed_back(again):
        raise p.Broken(
            "G8",
            "the memory was handed back once and then never again, with nobody having "
            "judged it. Whoever was not watching that one run is never told, and the "
            "whole guarantee reduces to `you had to be looking at the right moment`",
        )


@scenario(
    "G8",
    "the instrument is recaptured: does that answer a judgement nobody made?",
    varies=("world",),
    needs=("has_axes",),
)
def recapturing_an_instrument_does_not_answer_an_open_judgement(c):
    address, _ = c.put("why.md")
    c.bind(address)
    c.settle()

    c.happen("reading_changed")
    p.loud(c.gmr.check(), "the signal moved")

    c.gmr.recapture("the instrument changed", keys=[c.world.signal])

    after = c.gmr.check()
    if address not in c.gmr.handed_back(after):
        raise p.Broken(
            "G8",
            "recapturing pinned the world as it is now and the outstanding judgement "
            "went quiet with it. Nobody looked, nothing was sealed, and the anchor now "
            "reads as settled",
        )


@scenario(
    "G8",
    "the rules put the obligation away themselves: is it still not discarded?",
    varies=("world",),
    needs=("can_be_uncooperative",),
)
def an_obligation_the_rules_put_away_is_still_not_discarded(c):
    c.world.uncooperative(c.repo)
    c.gmr.declare()
    c.gmr.adjudicate(c.world.signal, "take the rules as declared", criteria=True)
    address, _ = c.put("why.md")
    c.bind(address)
    c.gmr.check()
    c.gmr.check()

    c.happen("reading_changed")
    p.loud(c.gmr.check(), "the signal moved")
    quiet = c.gmr.check()

    if address in c.gmr.handed_back(quiet):
        raise RuntimeError("these rules did not put the obligation away; the fixture proves nothing")

    refused = c.gmr.recapture("the instrument changed", keys=[c.world.signal])
    if refused.code == 0:
        raise p.Broken(
            "G8",
            "the rules put the obligation away, so delivery went quiet -- that is the "
            "domain's call. But recapturing then pinned the world with a judgement still "
            "unanswered and nothing recorded that one had been owed. Whether a memory is "
            "handed over now is a question about the present; whether one was ever owed "
            "and never answered is a question about the past, and the journal already "
            "holds the answer",
        )


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

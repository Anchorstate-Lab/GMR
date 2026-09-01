#!/usr/bin/env python3
"""The acceptance gate: does GMR keep its promise, in every world and every store?

Invoked by acceptance.sh once a tarball has been built and installed, so what is
exercised is the shipped binary rather than a debug build with the repository's
own bootstrap data lying around it.

    python3 tools/acceptance.py --binary <path to gmr>

What it asserts lives in `accept/spec.py`, in the vocabulary of the promise and
in no domain's words. How it reaches the CLI lives in `accept/driver.py` and
nowhere else. Which cells must exist lives in `accept/matrix.py`. Whether the
assertions still have teeth lives in `accept/mutations.py`.

The exit code follows the rule this repository already applies to `doctor`: red
is decided by who can fix it. A broken promise, an unexercised dimension and a
mutation that nothing caught are all somebody's to fix here, so they are red.
Wording is checked and reported and never red — an assertion that fails when a
sentence is reworded teaches its maintainer to soften assertions, and a softened
assertion is how a gate spends years green while testing nothing.
"""

import argparse
import concurrent.futures
import pathlib
import sys
import subprocess
import time
import traceback

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from accept import matrix, mutations, predicates, spec  # noqa: E402
from accept.cell import Cell  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent


class Outcome:
    def __init__(self, cellspec, verdict, detail=None, guarantee=None, seconds=0.0):
        self.spec, self.verdict, self.detail = cellspec, verdict, detail
        self.guarantee = guarantee or cellspec.scenario["guarantee"]
        self.seconds = seconds

    @property
    def red(self):
        return self.verdict in ("broken", "exploded")


def run_cell(binary, cellspec):
    declined = cellspec.declined
    if declined:
        return Outcome(cellspec, "declined", declined)
    started = time.time()
    cell = Cell(binary, type(cellspec.world)(), type(cellspec.store)(), cellspec.id)
    try:
        cell.build()
        cellspec.scenario["run"](cell)
        return Outcome(cellspec, "kept", seconds=time.time() - started)
    except predicates.Broken as b:
        return Outcome(cellspec, "broken", str(b), b.guarantee, time.time() - started)
    except Exception:
        return Outcome(
            cellspec, "exploded", traceback.format_exc(limit=4), seconds=time.time() - started
        )
    finally:
        cell.close()


def report(outcomes, faults, seconds):
    by_guarantee = {}
    for o in outcomes:
        by_guarantee.setdefault(o.guarantee, []).append(o)

    print()
    for g in sorted(by_guarantee):
        rows = by_guarantee[g]
        broken = [o for o in rows if o.red]
        kept = [o for o in rows if o.verdict == "kept"]
        declined = [o for o in rows if o.verdict == "declined"]
        mark = "×" if broken else "·"
        print(
            f"{mark} {g}   {len(kept)} kept   {len(broken)} broken"
            + (f"   {len(declined)} declined" if declined else "")
        )
        for o in broken:
            print(f"    {o.spec.id}")
            for line in (o.detail or "").strip().splitlines():
                print(f"      {line}")

    if faults:
        print("\n× matrix")
        for f in faults:
            print(f"    {f}")

    total_red = sum(1 for o in outcomes if o.red) + len(faults)
    kept = sum(1 for o in outcomes if o.verdict == "kept")
    declined = sum(1 for o in outcomes if o.verdict == "declined")
    print(
        f"\n{kept} kept · {total_red} broken · {declined} declined "
        f"· {len(outcomes)} cells in {seconds:.0f}s"
    )
    return total_red


def rebuild():
    r = subprocess.run(
        ["cargo", "build", "--release", "-p", "gmr-cli"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return r.returncode == 0, r.stderr[-2000:]


def run_mutations(binary, jobs):
    """Land a known break and insist the promises notice.

    A promise no mutation here can break is a promise nothing is really
    checking, so a mutation that lands with every cell still green is the gate
    reporting on itself.
    """
    faults = []

    # The baseline is taken once, on a tree with no mutation in it. Taking it
    # inside the loop read the *previous* mutation's binary, because a revert
    # restores the source and not the artefact -- which is the same class of
    # mistake this layer exists to catch, made by this layer.
    ok, err = rebuild()
    if not ok:
        return [f"the tree does not build before any mutation was applied\n{err}"]

    # What the files held before anything was applied. `git diff` cannot tell a
    # leftover mutation from the work in progress that is being tested, and a
    # gate that cries wolf during ordinary work gets ignored during real work.
    before = {m["file"]: (ROOT / m["file"]).read_text() for m in mutations.live()}

    targets = {}
    for m in mutations.live():
        targets[m["id"]] = [
            c
            for c in matrix.expand(spec.SCENARIOS)
            if c.scenario["id"] in m["breaks"] and not c.declined
        ]
    union = {c.id: c for cells in targets.values() for c in cells}
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
        standing = list(pool.map(lambda c: run_cell(binary, c), union.values()))
    already = {o.spec.id for o in standing if o.red}

    for m in mutations.live():
        print(f"\n── mutation: {m['id']}")
        cells = targets[m["id"]]
        # A promise that is already broken cannot testify. Land a mutation on top
        # of one and it stays red for the reason it was red before, which reads
        # exactly like the sentinel having caught something.
        blind = [c for c in cells if c.id in already]
        if blind and len(blind) == len(cells):
            faults.append(
                f"{m['id']}: every promise it aims at is already broken "
                f"({', '.join(m['breaks'])}), so landing it would prove nothing. "
                "This sentinel is not guarding anything until those go green"
            )
            print("   inconclusive — the promises it aims at are already broken")
            continue

        original = mutations.apply(ROOT, m)
        try:
            ok, err = rebuild()
            if not ok:
                faults.append(f"{m['id']}: the mutated tree does not build\n{err}")
                continue
            live_cells = [c for c in cells if c.id not in already]
            with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
                outcomes = list(pool.map(lambda c: run_cell(binary, c), live_cells))
            caught = [o for o in outcomes if o.red]
            print(f"   {len(caught)}/{len(outcomes)} cells noticed")
            if not caught:
                faults.append(
                    f"{m['id']}: nothing noticed. {m['why']} — "
                    f"the assertions behind {', '.join(m['breaks'])} have gone hollow"
                )
        finally:
            mutations.revert(ROOT, m, original)

    ok, err = rebuild()
    if not ok:
        faults.append(f"the tree does not build after reverting every mutation\n{err}")
    left = [f for f, text in before.items() if (ROOT / f).read_text() != text]
    if left:
        faults.append(f"a mutation was left in the tree: {' '.join(left)}")
    return faults


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default=str(ROOT / "target" / "release" / "gmr"))
    ap.add_argument("--only", default=None, help="substring filter over cell ids")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--list", action="store_true")
    ap.add_argument(
        "--mutations",
        action="store_true",
        help="land known breaks and insist the promises notice; rebuilds the binary",
    )
    ap.add_argument(
        "--sentinels-only",
        action="store_true",
        help="judge only whether the assertions still have teeth, not whether the "
        "promises hold — so a broken promise and a hollow one stay separate answers",
    )
    args = ap.parse_args()

    cells = matrix.expand(spec.SCENARIOS)
    faults = matrix.exhaustive(cells)
    if args.only:
        cells = [c for c in cells if args.only in c.id]

    if args.list:
        for c in cells:
            print(f"{c.scenario['guarantee']:4} {c.id}")
        print(f"\n{len(cells)} cells")
        return 0

    if not pathlib.Path(args.binary).exists():
        print(f"accept: no binary at {args.binary}", file=sys.stderr)
        return 2

    if args.sentinels_only:
        hollow = run_mutations(args.binary, args.jobs)
        for h in hollow:
            print(f"\n× {h}")
        print(f"\nSENTINELS {'HOLLOW' if hollow else 'SHARP'} count={len(mutations.live())}")
        return 1 if hollow else 0

    started = time.time()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        outcomes = list(pool.map(lambda c: run_cell(args.binary, c), cells))

    red = report(outcomes, faults, time.time() - started)

    if args.mutations:
        hollow = run_mutations(args.binary, args.jobs)
        if hollow:
            print("\n× sentinels")
            for h in hollow:
                print(f"    {h}")
        red += len(hollow)
    print(f"\nACCEPTANCE {'BROKEN' if red else 'KEPT'} cells={len(outcomes)} promises={len(spec.SCENARIOS)}")
    return 1 if red else 0


if __name__ == "__main__":
    sys.exit(main())

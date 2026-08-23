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
import time
import traceback

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from accept import matrix, predicates, spec  # noqa: E402
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default=str(ROOT / "target" / "release" / "gmr"))
    ap.add_argument("--only", default=None, help="substring filter over cell ids")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--list", action="store_true")
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

    started = time.time()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        outcomes = list(pool.map(lambda c: run_cell(args.binary, c), cells))

    red = report(outcomes, faults, time.time() - started)
    print(f"\nACCEPTANCE {'BROKEN' if red else 'KEPT'} cells={len(outcomes)} promises={len(spec.SCENARIOS)}")
    return 1 if red else 0


if __name__ == "__main__":
    sys.exit(main())

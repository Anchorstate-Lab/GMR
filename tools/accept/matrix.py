"""Layer 2 — the cartesian product, and the rules that keep it honest.

Coverage that depends on somebody remembering to add a test decays, always and
in one direction. The old suite left eleven verbs never invoked once, not
because anybody decided they did not matter but because adding a verb carried no
obligation to add a case. So the dimensions here are read off the registries the
product itself keeps, and a value that appears in a registry and not in the
matrix turns the gate red.

Two rules live here rather than in prose:

  * every world must be able to express every universal event — a signal source
    that cannot change, cannot be noisy and cannot cease is not a signal source;
  * at least one world must not derive its signal from the source tree.

The second is the load-bearing one. It is the only mechanism in the repository
that fails when GMR quietly becomes a coding product.
"""

from . import stores as store_registry
from . import worlds as world_registry
from .worlds import UNIVERSAL


class Cellspec:
    def __init__(self, scenario, world, store):
        self.scenario, self.world, self.store = scenario, world, store

    @property
    def id(self):
        return f"{self.scenario['id']}[{self.world.name}/{self.store.name}]"

    @property
    def declined(self):
        for need in self.scenario["needs"]:
            if not getattr(self.store, need, getattr(self.world, need, False)):
                return f"{self.store.name} cannot {need.replace('_', ' ')}"
        return None


def expand(scenarios, worlds=None, stores=None):
    worlds = worlds if worlds is not None else world_registry.ALL
    stores = stores if stores is not None else store_registry.ALL
    out = []
    for s in scenarios:
        ws = worlds if "world" in s["varies"] else worlds[:1]
        ss = stores if "store" in s["varies"] else stores[:1]
        for w in ws:
            for st in ss:
                out.append(Cellspec(s, w, st))
    return out


def exhaustive(specs, worlds=None, stores=None):
    """Every registered dimension is actually exercised, and the suite is not all code."""
    worlds = worlds if worlds is not None else world_registry.ALL
    stores = stores if stores is not None else store_registry.ALL
    faults = []

    for w in worlds:
        missing = [e for e in UNIVERSAL if not w.can(e)]
        if missing:
            faults.append(f"world `{w.name}` cannot express {', '.join(missing)}")

    if not any(not w.derives_from_source for w in worlds):
        faults.append(
            "every registered world reads the source tree. A suite made only of parse "
            "trees would say GMR is a coding tool more convincingly than any doc says "
            "it is not — register a signal source that no extractor can derive"
        )

    ran_worlds = {c.world.name for c in specs if not c.declined}
    ran_stores = {c.store.name for c in specs if not c.declined}
    for w in worlds:
        if w.name not in ran_worlds:
            faults.append(f"world `{w.name}` is registered but no scenario runs against it")
    for s in stores:
        if s.name not in ran_stores:
            faults.append(
                f"store `{s.name}` is registered but no scenario runs against it — "
                "a store nothing checks is a store whose promise nobody has read"
            )
    return faults

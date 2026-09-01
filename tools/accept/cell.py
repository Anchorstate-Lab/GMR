"""One cell of the matrix: a world, a store, and a runtime instance of its own.

Every scenario gets a cell nobody else has touched. The suite this replaces ran
twenty-odd steps against one accumulating repository: the order was load-bearing
and undeclared, the first failure hid every step after it, and its final
assertion rested on a binding table nineteen earlier steps had shaped. A
scenario that cannot be run alone cannot be trusted when it passes.
"""

import shutil
import subprocess
import tempfile
from pathlib import Path

from . import driver

KEEP = (".git", ".anchor")


def _git(repo, *args):
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


def _commit(repo, message):
    _git(repo, "add", "-A")
    _git(repo, "-c", "user.email=a@b", "-c", "user.name=t", "commit", "-qm", message)


class Cell:
    """A built fixture: one signal source, one memory store, one runtime."""

    def __init__(self, binary, world, store, name, work=None):
        self.binary, self.world, self.store, self.name = binary, world, store, name
        self.root = Path(tempfile.mkdtemp(prefix="gmr-accept-"))
        self.repo = self.root / "repo"
        self.repo.mkdir(parents=True)
        self.work = Path(work) if work else self.root / "work"
        self.work.mkdir(parents=True, exist_ok=True)
        self.shared_work = work is not None
        self.gmr = None
        self._n = 0

    def build(self):
        self.world.build(self.repo)
        driver.Gmr(self.binary, self.repo).init()
        self.env = self.store.prepare(self.repo, self.work)
        self.gmr = driver.Gmr(self.binary, self.repo, self.env)
        self.world.declare(self.gmr, self.repo)
        return self

    def close(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def __enter__(self):
        return self.build()

    def __exit__(self, *exc):
        self.close()
        return False

    def sibling(self):
        """An independent cell of the same kind, for scenarios comparing outcomes."""
        return Cell(self.binary, type(self.world)(), type(self.store)(), self.name)

    # ── memories ────────────────────────────────────────────────────────────

    def marker(self, tag="m"):
        self._n += 1
        return f"a judgment nothing in the signal can state for itself :: {tag}{self._n}"

    def put(self, mid, text=None, tag="m"):
        body = text if text is not None else self.marker(tag)
        return self.store.write(self.repo, self.work, mid, body + "\n"), body

    def rewrite(self, mid, text=None):
        body = text if text is not None else self.marker("rewritten")
        return self.store.rewrite(self.repo, self.work, mid, body + "\n"), body

    def drop(self, mid):
        self.store.delete(self.repo, self.work, mid)

    def bind(self, address, signals=None):
        return self.gmr.bind(address, anchors=signals or [self.world.signal])

    def subscribe(self, mid, axes):
        """A memory that says for itself which axes should wake it.

        Only a store whose records live in the repository can express this: the
        subscription is read out of the record's own frontmatter, which is why
        the promise that uses it declines every other store rather than
        pretending to have covered it.
        """
        body = "---\nabout: {}\nwatch: [{}]\n---\n\n{}\n".format(
            self.world.signal, ", ".join(axes), self.marker("narrow")
        )
        address, _ = self.put(mid, text=body.rstrip("\n"))
        self.gmr.bind(address, anchors=[self.world.signal])
        return address

    def reading(self, signal=None):
        """The address of the reading this signal is showing right now.

        It is what `gmr read --json` hands an agent, and what an agent has to
        carry back into `said` for anything to be able to tell a conclusion
        built through the anchor from one built beside it.
        """
        res = self.gmr.read(signal or self.world.signal)
        seen = res.body if isinstance(res.body, dict) else res.body[0]
        return seen.get("fact_address")

    def without_store(self):
        """The same instance, with this store pointed somewhere that is not there."""
        if not self.store.env_key:
            raise RuntimeError(f"{self.store.name} cannot be made unreachable")
        return self.gmr.with_env(**{self.store.env_key: str(self.root / "no-such-store")})

    # ── driving the world ───────────────────────────────────────────────────

    def _stash(self):
        stash = self.root / "stash"
        shutil.rmtree(stash, ignore_errors=True)
        shutil.copytree(self.repo, stash, ignore=shutil.ignore_patterns(*KEEP))

    def happen(self, event):
        self._stash()
        getattr(self.world, event)(self.repo)
        if self.store.name == "git":
            _commit(self.repo, event)

    def revert(self):
        """Put the world back exactly as it was before the last event."""
        for item in self.repo.iterdir():
            if item.name in KEEP:
                continue
            shutil.rmtree(item) if item.is_dir() else item.unlink()
        for item in (self.root / "stash").iterdir():
            dest = self.repo / item.name
            shutil.copytree(item, dest) if item.is_dir() else shutil.copy2(item, dest)
        if self.store.name == "git":
            _commit(self.repo, "put it back")

    def settle(self, why="take this reading as the baseline"):
        self.gmr.check()
        if self.gmr.check().code == 1:
            self.gmr.adjudicate(self.world.signal, why)
            self.gmr.check()
        return self

    # ── what the promise rests on, and carrying it across instances ─────────

    def summary(self):
        """Everything the promise is made of, in a form two instances can compare."""
        signals, memories, pending, provenance = set(), set(), set(), set()
        for key in self.gmr.signals():
            signals.add(key)
            if self.gmr.axes_set(key):
                pending.add(key)
            for address in self.gmr.bound_addresses(key):
                memories.add(address)
                pv = self.gmr.provenance_of(key, address) or {}
                provenance.add(
                    (key, address, tuple(pv.get("sources") or ()), pv.get("bound_version"))
                )
        return {
            "signals": signals,
            "memories": memories,
            "pending": pending,
            "provenance": provenance,
        }

    def migrate(self):
        """Carry this instance into a fresh one the only way a user can.

        A runtime instance is not a clone of a repository and does not pretend to
        be one: the journal never travelled with the source tree and is not meant
        to. Export and import are the whole channel by which memories cross from
        one instance to the next, which makes them the lifeline of the promise
        rather than two corner verbs — and they were the two verbs the old suite
        never once invoked.
        """
        dump = self.root / "carry.jsonl"
        self.gmr.export(dump)

        fresh = Cell(self.binary, self.world, self.store, self.name + " (migrated)", work=self.work)
        fresh.world.build(fresh.repo)
        carried = self.repo / "memories"
        if carried.is_dir() and any(carried.iterdir()):
            shutil.copytree(carried, fresh.repo / "memories", dirs_exist_ok=True)
            _commit(fresh.repo, "carry the records over")
        driver.Gmr(self.binary, fresh.repo).init()
        fresh.env = fresh.store.prepare(fresh.repo, fresh.work)
        fresh.gmr = driver.Gmr(self.binary, fresh.repo, fresh.env)
        fresh.world.recipes(fresh.repo)
        fresh.gmr.import_(dump)
        self._migrated = fresh
        return fresh

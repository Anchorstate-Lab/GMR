"""One cell of the matrix: a world, a store, and a fresh runtime instance.

Every scenario gets its own cell. Nothing is shared between them and nothing
carries over. The suite this replaces ran twenty-odd steps against one
accumulating repository, which made the order load-bearing without anybody
declaring it, hid every later step behind the first failure, and left its last
assertion depending on a binding table nineteen earlier steps had shaped. A
scenario that cannot be run alone cannot be trusted when it passes.
"""

import shutil
import subprocess
import tempfile
from pathlib import Path

from . import driver


class Cell:
    """A built fixture: one signal source, one memory store, one runtime."""

    def __init__(self, binary, world, store, name):
        self.binary, self.world, self.store, self.name = binary, world, store, name
        self.root = Path(tempfile.mkdtemp(prefix="gmr-accept-"))
        self.repo = self.root / "repo"
        self.work = self.root / "work"
        self.repo.mkdir(parents=True)
        self.work.mkdir(parents=True)
        self.gmr = None
        self._n = 0

    def build(self):
        self.world.build(self.repo)
        base = driver.Gmr(self.binary, self.repo)
        base.init()
        env = self.store.prepare(self.repo, self.work)
        self.gmr = driver.Gmr(self.binary, self.repo, env)
        self.world.declare(self.gmr, self.repo)
        return self

    def close(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def __enter__(self):
        return self.build()

    def __exit__(self, *exc):
        self.close()
        return False

    # ── memories ────────────────────────────────────────────────────────────

    def marker(self, tag="m"):
        self._n += 1
        return f"a judgment nothing in the signal can state for itself :: {tag}{self._n}"

    def put(self, mid, text=None, tag="m"):
        """Write a record into this cell's store and return (address, its text)."""
        body = text if text is not None else self.marker(tag)
        return self.store.write(self.repo, self.work, mid, body + "\n"), body

    def rewrite(self, mid, text=None):
        body = text if text is not None else self.marker("rewritten")
        return self.store.rewrite(self.repo, self.work, mid, body + "\n"), body

    def drop(self, mid):
        self.store.delete(self.repo, self.work, mid)

    def bind(self, address, signals=None):
        return self.gmr.bind(address, anchors=signals or [self.world.signal])

    # ── driving the world ───────────────────────────────────────────────────

    def happen(self, event):
        getattr(self.world, event)(self.repo)
        if self.store.name == "git":
            subprocess.run(["git", "add", "-A"], cwd=self.repo, check=True, capture_output=True)
            subprocess.run(
                ["git", "-c", "user.email=a@b", "-c", "user.name=t", "commit", "-qm", event],
                cwd=self.repo,
                check=True,
                capture_output=True,
            )

    def settle(self, why="settle"):
        """Take the current reading as the baseline, so later moves are real moves."""
        self.gmr.check()
        res = self.gmr.check()
        if res.code == 1:
            self.gmr.adjudicate(self.world.signal, why)
            self.gmr.check()
        return self

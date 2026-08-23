"""Where a memory lives. GMR's claim is that it does not care; a matrix proves it.

Every scenario that touches a memory runs against every store here, asserting
exactly the same things. That is what makes "the store is the user's business"
an invariant rather than a slogan. Testing rewriting only against one store and
absence only against another is a jigsaw, not a proof — and a jigsaw is how the
git store ended up being the one path no test ever exercised for drift.

A store that cannot do something says so by not implementing it, and `matrix.py`
records the cell as declined rather than letting it vanish.
"""

import abc
import subprocess

DESK_FETCH = """#!/bin/sh
[ -d "$DESK" ] || { echo "the desk is not mounted" >&2; exit 3; }
id=$(printf '%s' "$GMR_POSITION" | sed 's/.*"id":"\\([^"]*\\)".*/\\1/')
file="$DESK/$id"
[ -f "$file" ] || { printf 'null'; exit 0; }
printf '{"text":"%s"}' "$(sed 's/"/\\\\"/g' "$file" | tr -d '\\n')"
"""

DESK_LIST = """#!/bin/sh
[ -d "$DESK" ] || { echo "the desk is not mounted" >&2; exit 3; }
printf '{"records":['
first=1
for f in "$DESK"/*; do
  [ -f "$f" ] || continue
  [ $first -eq 1 ] || printf ','
  first=0
  printf '{"id":"%s","text":"%s"}' "$(basename "$f")" "$(sed 's/"/\\\\"/g' "$f" | tr -d '\\n')"
done
printf ']}'
"""

DESK_TOML = """[provider.desk]
fetch = "scripts/desk-fetch.sh"
list = "scripts/desk-list.sh"
ids = "readable"
"""


class Store(abc.ABC):
    name = None
    prefix = None
    env_key = None

    # Whether a record kept here can say for itself which axes should wake it.
    # Only a store whose records live in the repository can: the subscription is
    # read out of the note's own frontmatter.
    per_note_watch = False

    @property
    def store_can_vanish(self):
        """Whether this store lives somewhere that can stop answering.

        A store kept inside the repository cannot go unreachable, so the
        scenario about unreachability declines that cell rather than
        pretending to have run it.
        """
        return self.env_key is not None

    def prepare(self, repo, work):
        """Make the store exist. Returns env the driver must carry."""
        return {}

    def address(self, mid):
        return f"{self.prefix}:{mid}"

    @abc.abstractmethod
    def write(self, repo, work, mid, text):
        """Put a record there and return its address."""

    def rewrite(self, repo, work, mid, text):
        return self.write(repo, work, mid, text)

    @abc.abstractmethod
    def delete(self, repo, work, mid):
        """The store now says this record is gone."""


class Git(Store):
    name = "git"
    prefix = "git"
    per_note_watch = True

    def address(self, mid):
        return f"git:memories/{mid}"

    def _commit(self, repo, message):
        subprocess.run(["git", "add", "-A"], cwd=repo, check=True, capture_output=True)
        subprocess.run(
            ["git", "-c", "user.email=a@b", "-c", "user.name=t", "commit", "-qm", message],
            cwd=repo,
            check=True,
            capture_output=True,
        )

    def write(self, repo, work, mid, text):
        d = repo / "memories"
        d.mkdir(parents=True, exist_ok=True)
        (d / mid).write_text(text)
        self._commit(repo, f"record {mid}")
        return self.address(mid)

    def delete(self, repo, work, mid):
        (repo / "memories" / mid).unlink()
        self._commit(repo, f"drop {mid}")


class ClaudeCode(Store):
    name = "claude-code"
    prefix = "claude-code"
    env_key = "GMR_CLAUDE_MEMORY_DIR"

    def _dir(self, work):
        d = work / "claude-memory"
        d.mkdir(parents=True, exist_ok=True)
        return d

    def prepare(self, repo, work):
        return {"GMR_CLAUDE_MEMORY_DIR": str(self._dir(work))}

    def write(self, repo, work, mid, text):
        (self._dir(work) / mid).write_text(text)
        return self.address(mid)

    def delete(self, repo, work, mid):
        (self._dir(work) / mid).unlink()


class Desk(Store):
    """A store taught to the binary by a recipe, with no Rust anywhere.

    Its presence in the matrix is what holds the claim that a store can be added
    without a compiler honest — every state a compiled provider reaches, this one
    has to reach too.
    """

    name = "desk"
    prefix = "desk"
    env_key = "DESK"

    def _dir(self, work):
        d = work / "desk"
        d.mkdir(parents=True, exist_ok=True)
        return d

    def prepare(self, repo, work):
        scripts = repo / "scripts"
        scripts.mkdir(parents=True, exist_ok=True)
        for name, body in (("desk-fetch.sh", DESK_FETCH), ("desk-list.sh", DESK_LIST)):
            p = scripts / name
            p.write_text(body)
            p.chmod(0o755)
        (repo / ".anchor" / "providers.toml").write_text(DESK_TOML)
        return {"DESK": str(self._dir(work))}

    def write(self, repo, work, mid, text):
        (self._dir(work) / mid).write_text(text)
        return self.address(mid)

    def delete(self, repo, work, mid):
        (self._dir(work) / mid).unlink()


ALL = [Git(), ClaudeCode(), Desk()]

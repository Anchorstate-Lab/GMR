"""A signal source that is not code and cannot be read out of any parse tree.

Which commit is running on staging is a fact about the world, not about this
repository. Nothing in the source says it, no extractor can derive it, and a
memory about it — "the 30 minutes is the CDN cache window, not a security
choice" — is exactly the kind of judgment GMR exists to keep honest.

This world is not decoration and it is not optional. It is the only thing in
the suite that fails when GMR quietly becomes a coding tool, so `matrix.py`
turns the gate red if every registered world reads a parse tree.
"""

import subprocess

from . import base

PROBES = """[script.deploy-sha]
run = "scripts/deploy.sh"
obs = { schema = "gmr.probe-deploy.v1", at = [], facts = ["sha"] }
"""

# The script is the instrument, so it must never change: rewriting it would
# change the probe's earned identity, and every reading taken before would stop
# being comparable. What the instrument reads sits beside it in a file, exactly
# as an extractor reads a source tree it does not own.
READER = """#!/bin/sh
env=$(printf '%s' "$GMR_POSITION" | sed 's/.*"env":"\\([^"]*\\)".*/\\1/')
f="$GMR_ROOT/deploy-state/$env"
[ -f "$f" ] || f="deploy-state/$env"
[ -f "$f" ] || { printf 'null'; exit 0; }
cat "$f"
"""

ANCHOR = """
[[anchor]]
key = "{key}"
probe = "deploy-sha"
position = {{ env = "{env}" }}
rules = [
  'not exists(state.sha) => {{ position: state.position, sha: obs.sha, status: "captured" }}',
  'obs.sha != state.sha => {{ position: state.position, sha: obs.sha, was: state.sha, status: "redeployed" }}',
]
watch = 'state.status != "captured"'
"""

STAGING = "staging"


class World(base.World):
    name = "deploy"
    derives_from_source = False
    has_axes = False
    swappable_instrument = True
    expresses = base.UNIVERSAL

    @property
    def coordinate(self):
        return None

    @property
    def signal(self):
        return f"deploy::{STAGING}"

    def build(self, repo):
        (repo / "scripts").mkdir(parents=True, exist_ok=True)
        p = repo / "scripts" / "deploy.sh"
        p.write_text(READER)
        p.chmod(0o755)
        self._reads(repo, STAGING, '{"sha":"a1b2c3d"}')
        (repo / "README.md").write_text(
            "a service whose deployments nothing in this tree records\n"
        )
        subprocess.run(["git", "init", "-q", "."], cwd=repo, check=True, capture_output=True)
        subprocess.run(["git", "add", "-A"], cwd=repo, check=True, capture_output=True)
        subprocess.run(
            ["git", "-c", "user.email=a@b", "-c", "user.name=t", "commit", "-qm", "init"],
            cwd=repo,
            check=True,
            capture_output=True,
        )

    def _reads(self, repo, env, payload):
        d = repo / "deploy-state"
        d.mkdir(parents=True, exist_ok=True)
        (d / env).write_text(payload)

    def recipes(self, repo):
        (repo / ".anchor" / "probes.toml").write_text(PROBES)
        declared = repo / ".anchor" / "anchors.toml"
        if not declared.exists():
            declared.write_text(ANCHOR.format(key=self.signal, env=STAGING))

    def declare(self, gmr, repo):
        self.recipes(repo)
        gmr.declare()

    # ── events ──────────────────────────────────────────────────────────────

    def reading_changed(self, repo):
        self._reads(repo, STAGING, '{"sha":"9f8e7d6"}')

    def noise(self, repo):
        self._reads(repo, STAGING, '{  "sha" :  "a1b2c3d"  }')

    def ceased(self, repo):
        self._reads(repo, STAGING, "null")

    def swap_instrument(self, repo):
        """Same readings, a different thing doing the reading."""
        p = repo / "scripts" / "deploy.sh"
        p.write_text(READER.replace("cat \"$f\"", "cat \"$f\" | cat"))
        p.chmod(0o755)

    def many(self, repo, n):
        blocks = [ANCHOR.format(key=self.signal, env=STAGING)]
        keys = []
        for i in range(n):
            env = f"bulk{i}"
            key = f"deploy::{env}"
            blocks.append(ANCHOR.format(key=key, env=env))
            self._reads(repo, env, '{"sha":"0000%03d"}' % i)
            keys.append(key)
        (repo / ".anchor" / "anchors.toml").write_text("".join(blocks))
        return keys

    def uncooperative(self, repo):
        rules_back = ANCHOR.format(key=self.signal, env=STAGING).replace(
            "]\nwatch",
            "  'state.status == \"redeployed\" => { position: state.position, sha: state.sha,"
            " status: \"captured\" }',\n]\nwatch",
        )
        (repo / ".anchor" / "anchors.toml").write_text(rules_back)

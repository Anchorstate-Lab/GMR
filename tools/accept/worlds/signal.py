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

ANCHORS = """[[anchor]]
key = "deploy::staging"
probe = "deploy-sha"
position = { env = "staging" }
rules = [
  'not exists(state.sha) => { position: state.position, sha: obs.sha, status: "captured" }',
  'obs.sha != state.sha => { position: state.position, sha: obs.sha, was: state.sha, status: "redeployed" }',
]
"""


def _script(body):
    return "#!/bin/sh\n" + body


class World(base.World):
    name = "deploy"
    expresses = base.UNIVERSAL

    @property
    def coordinate(self):
        return None

    @property
    def signal(self):
        return "deploy::staging"

    def build(self, repo):
        (repo / "scripts").mkdir(parents=True, exist_ok=True)
        self._emit(repo, '{"sha":"a1b2c3d"}')
        (repo / "README.md").write_text("a service whose deployments nothing in here records\n")
        subprocess.run(["git", "init", "-q", "."], cwd=repo, check=True, capture_output=True)
        subprocess.run(["git", "add", "-A"], cwd=repo, check=True, capture_output=True)
        subprocess.run(
            ["git", "-c", "user.email=a@b", "-c", "user.name=t", "commit", "-qm", "init"],
            cwd=repo,
            check=True,
            capture_output=True,
        )

    def declare(self, gmr, repo):
        (repo / ".anchor" / "probes.toml").write_text(PROBES)
        (repo / ".anchor" / "anchors.toml").write_text(ANCHORS)
        gmr.declare()

    def _emit(self, repo, payload):
        p = repo / "scripts" / "deploy.sh"
        p.write_text(_script(f"printf '{payload}\\n'\n"))
        p.chmod(0o755)

    # ── events ──────────────────────────────────────────────────────────────

    def reading_changed(self, repo):
        self._emit(repo, '{"sha":"9f8e7d6"}')

    def noise(self, repo):
        self._emit(repo, '{  "sha" :  "a1b2c3d"  }')

    def ceased(self, repo):
        self._emit(repo, "null")

"""The coding world: a signal source read out of a parse tree.

This is the only domain GMR ships today, which is exactly why it must not be
the only world in the matrix. It is one instantiation of `base.World`, not the
definition of one.
"""

import subprocess

from . import base

SUBJECT = "createSession"
OTHER_FILE = "src/moved.ts"


def _commit(repo, message):
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True, capture_output=True)
    subprocess.run(
        ["git", "-c", "user.email=a@b", "-c", "user.name=t", "commit", "-qm", message],
        cwd=repo,
        check=True,
        capture_output=True,
    )


class World(base.World):
    name = "coding"
    expresses = base.UNIVERSAL + base.SHAPED

    @property
    def coordinate(self):
        return f"src/auth.ts#{SUBJECT}"

    def build(self, repo):
        (repo / "src").mkdir(parents=True, exist_ok=True)
        self._write(
            repo,
            """export function createSession(userId: string, ttl: number): Session {
  return { userId, ttl };
}
export const verify = (token: string) => token.length > 0;
""",
        )
        subprocess.run(["git", "init", "-q", "."], cwd=repo, check=True, capture_output=True)
        _commit(repo, "init")

    def _write(self, repo, body, other=None):
        (repo / "src" / "auth.ts").write_text(body)
        if other is not None:
            (repo / OTHER_FILE).write_text(other)

    # ── events ──────────────────────────────────────────────────────────────

    def reading_changed(self, repo):
        self._write(
            repo,
            """export function createSession(userId: string, ttl: number): Session {
  const started = Date.now();
  return { userId, ttl, started };
}
export const verify = (token: string) => token.length > 0;
""",
        )

    def noise(self, repo):
        self._write(
            repo,
            """export function createSession(
      userId: string,
      ttl: number,
): Session {
        // the shape of this object is fixed by the wire format
        return { userId, ttl };
}
export const verify = (token: string) => token.length > 0;
""",
        )

    def ceased(self, repo):
        self._write(repo, "export const verify = (token: string) => token.length > 0;\n")

    def identity_changed(self, repo):
        self._write(
            repo,
            """export function openSession(userId: string, ttl: number): Session {
  return { userId, ttl };
}
export const verify = (token: string) => token.length > 0;
""",
        )

    def location_changed(self, repo):
        self._write(
            repo,
            "export const verify = (token: string) => token.length > 0;\n",
            other="""export function createSession(userId: string, ttl: number): Session {
  return { userId, ttl };
}
""",
        )

    def neighborhood_changed(self, repo):
        self._write(
            repo,
            """export function issue(id: string): string { return id; }
export function createSession(userId: string, ttl: number): Session {
  return { userId, ttl };
}
export const verify = (token: string) => token.length > 0;
""",
        )

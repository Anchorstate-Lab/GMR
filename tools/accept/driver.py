"""Layer 1 — the only module that knows what the CLI looks like.

Everything above this file speaks the vocabulary of the promise: signals,
readings, memories, adjudication. Everything below is `gmr` argv. When the CLI
renames a verb, changes a flag or rewords a sentence, this file changes and
nothing else does. That isolation is the whole reason the layer exists: the
scenarios outlive the interface.

No caller above this file may match on prose. The methods here return parsed
structures; a string that appears in human output and nowhere in `--json` is a
wording check, not a gate.
"""

import json
import os
import subprocess


class CliError(RuntimeError):
    def __init__(self, argv, code, out, err):
        self.argv, self.code, self.out, self.err = argv, code, out, err
        super().__init__(f"gmr {' '.join(argv)} exited {code}\n{out}\n{err}")


class Result:
    """One invocation: its exit code, its text, and its parsed body if any."""

    def __init__(self, code, out, err, body=None):
        self.code, self.out, self.err, self.body = code, out, err, body

    @property
    def quiet(self):
        return self.code == 0

    @property
    def loud(self):
        return self.code == 1


class Gmr:
    """A `gmr` binary pointed at one repository."""

    def __init__(self, binary, repo, env=None):
        self.binary = str(binary)
        self.repo = str(repo)
        self.env = dict(env or {})

    def child(self, repo):
        return Gmr(self.binary, repo, self.env)

    def with_env(self, **kw):
        e = dict(self.env)
        for k, v in kw.items():
            if v is None:
                e.pop(k, None)
            else:
                e[k] = str(v)
        return Gmr(self.binary, self.repo, e)

    # ── invocation ──────────────────────────────────────────────────────────

    def _run(self, argv, json_out=False, check=True, drop_env=(), codes=(0, 1)):
        env = dict(os.environ)
        env.update(self.env)
        for k in drop_env:
            env.pop(k, None)
        full = [self.binary, "--repo", self.repo] + list(argv)
        if json_out:
            full.append("--json")
        p = subprocess.run(full, capture_output=True, text=True, env=env)
        if check and p.returncode not in codes:
            raise CliError(argv, p.returncode, p.stdout, p.stderr)
        body = None
        if json_out and p.stdout.strip():
            try:
                body = json.loads(p.stdout)
            except json.JSONDecodeError:
                body = None
        return Result(p.returncode, p.stdout, p.stderr, body)

    def raw(self, *argv, **kw):
        return self._run(list(argv), **kw)

    # ── setting the world up ────────────────────────────────────────────────

    def init(self):
        return self._run(["init"])

    def declare(self, coordinate=None, record=None, memory=None):
        argv = ["anchor"]
        if coordinate:
            argv.append(coordinate)
        if record:
            argv += ["--record", record]
        if memory:
            argv += ["-m", memory]
        return self._run(argv)

    def sync(self):
        return self._run(["sync"])

    def requeue(self, key):
        return self._run(["requeue", key])

    # ── asking whether anything moved ───────────────────────────────────────

    def check(self, key=None, budget_ms=None):
        argv = ["check"] + ([key] if key else [])
        if budget_ms is not None:
            argv = ["--probe-budget-ms", str(budget_ms)] + argv
        return self._run(argv, json_out=True)

    def observe(self, key=None):
        return self._run(["observe"] + ([key] if key else []), json_out=True)

    def sweep(self, budget_ms=None):
        argv = ["pass"]
        if budget_ms is not None:
            argv = ["--probe-budget-ms", str(budget_ms)] + argv
        return self._run(argv, json_out=True)

    def read(self, key=None):
        return self._run(["read"] + ([key] if key else []), json_out=True)

    def status(self, key=None):
        return self._run(["status"] + ([key] if key else []), json_out=True)

    def doctor(self, drop_env=()):
        return self._run(["doctor"], json_out=True, drop_env=drop_env)

    def health(self, key=None):
        return self._run(["health"] + ([key] if key else []), json_out=True)

    # ── memories and their addresses ────────────────────────────────────────

    def memories(self, provider=None):
        argv = ["memories"] + (["--provider", provider] if provider else [])
        return self._run(argv, json_out=True)

    def bind(self, address, anchors=None, detach=False, provider=None):
        argv = ["bind", address]
        if anchors:
            argv += ["--anchors", ",".join(anchors)]
        if detach:
            argv.append("--detach")
        if provider:
            argv += ["--provider", provider]
        return self._run(argv)

    def attest(self, address, anchors, provider=None):
        argv = ["attest", address, "--anchors", ",".join(anchors)]
        if provider:
            argv += ["--provider", provider]
        return self._run(argv, json_out=True)

    def reaffirm(self, address, provider=None):
        argv = ["reaffirm", address] + (["--provider", provider] if provider else [])
        return self._run(argv)

    def cobound(self, address, provider=None):
        argv = ["cobound", address] + (["--provider", provider] if provider else [])
        return self._run(argv, json_out=True)

    # ── adjudication ────────────────────────────────────────────────────────

    def adjudicate(self, key, why, baseline=False, criteria=False, every=False):
        argv = ["accept"]
        if key:
            argv.append(key)
        if baseline:
            argv.append("--baseline")
        if criteria:
            argv.append("--criteria")
        if every:
            argv.append("--all")
        argv += ["--why", why]
        return self._run(argv)

    def recapture(self, why, keys=(), every=False):
        argv = ["rebase"] + list(keys)
        if every:
            argv.append("--all")
        argv += ["--why", why]
        return self._run(argv)

    def retire(self, key, why):
        return self._run(["close", key, "--why", why])

    # ── migration ───────────────────────────────────────────────────────────

    def export(self, out):
        return self._run(["export", "--out", str(out)])

    def import_(self, path):
        return self._run(["import", str(path)], codes=(0, 1, 2))

    # ── derived reads, still in the promise's vocabulary ────────────────────

    def handed_back(self, res=None):
        """Every memory address this run handed back, as a set."""
        body = res.body if res is not None else self.check().body
        out = set()
        for row in (body or {}).get("handed_back", []):
            out.update(row.get("memories", []))
        return out

    def moved_signals(self, res):
        return {row["anchor"] for row in (res.body or {}).get("handed_back", [])}

    @staticmethod
    def _addressed(reference):
        """`provider:external_id` — the one spelling every verb here takes back."""
        if isinstance(reference, str):
            return reference
        return f"{reference.get('provider')}:{reference.get('external_id')}"

    def _memory(self, key, address):
        body = self.read(key).body or []
        views = body if isinstance(body, list) else [body]
        for v in views:
            for m in v.get("memories", []):
                if self._addressed(m.get("reference")) == address:
                    return v, m
        return None, None

    def content_of(self, key, address):
        """The bytes of one memory, reached the way SKILL.md tells an agent to.

        `check` hands back addresses; `read` takes an anchor key and carries the
        content inside each memory's grounding. The joint between those two verbs
        is the agent's actual road to a memory, so the driver walks it rather than
        reaching past it into the store.
        """
        _, m = self._memory(key, address)
        return None if m is None else (m.get("grounding") or {}).get("content")

    def grounding_of(self, key, address):
        _, m = self._memory(key, address)
        return None if m is None else m.get("grounding")

    def provenance_of(self, key, address):
        v, m = self._memory(key, address)
        if m is None:
            return None
        return {
            "signal": v.get("key"),
            "sources": m.get("sources"),
            "asserted_at": m.get("asserted_at"),
            "bound_version": m.get("bound_version"),
            "reading_then": v.get("state"),
            "instrument": v.get("derivation"),
        }

    def bound_addresses(self, key):
        body = self.read(key).body or []
        views = body if isinstance(body, list) else [body]
        return {
            self._addressed(m.get("reference"))
            for v in views
            for m in v.get("memories", [])
        }

    def axes_set(self, key):
        """Which axes of the reading are standing, when the reading has axes.

        Not every signal source has axes — a deploy sha has a value, not a
        vector — so this is empty rather than absent for worlds without them.
        """
        body = self.read(key).body or []
        views = body if isinstance(body, list) else [body]
        for v in views:
            if v.get("key") != key:
                continue
            axes = (v.get("state") or {}).get("v")
            if isinstance(axes, dict):
                return frozenset(k for k, on in axes.items() if on is True)
        return frozenset()

    def doctor_buckets(self, key):
        """Every typed `doctor` bucket that names this signal or a memory on it."""
        body = self.doctor().body or {}
        named = set()
        for bucket, rows in body.items():
            if not isinstance(rows, list):
                continue
            for r in rows:
                text = r if isinstance(r, str) else (r.get("anchor") or r.get("note") or "")
                if text == key:
                    named.add(bucket)
        return frozenset(named)

    def fingerprint(self, key, address=None):
        """What this runtime says about one signal right now, as a structure.

        The point is discrimination, not description. A reader who cannot tell a
        renamed signal from a deleted one cannot choose between recapturing the
        anchor and retiring the memory, so `GMR reports the change` is only kept
        if different changes leave different fingerprints. Nothing in here is a
        sentence; every field comes out of `--json`.
        """
        chk = self.check(key)
        body = self.read(key).body or []
        views = body if isinstance(body, list) else [body]
        view = next((v for v in views if v.get("key") == key), {})
        facts = view.get("facts") or {}
        return {
            "exit": chk.code,
            "handed": address in self.handed_back(chk) if address else None,
            "status": view.get("status"),
            "sighting": view.get("sighting"),
            "axes": tuple(sorted(self.axes_set(key))),
            "doctor": tuple(sorted(self.doctor_buckets(key))),
            "found": facts.get("found"),
            "exact": facts.get("exact"),
            "matched": tuple(facts.get("matched") or ()),
            "missed": tuple(facts.get("missed") or ()),
        }

    # `candidates` and `roll` are deliberately left out of the fingerprint. They
    # describe what else happens to sit in the neighbourhood, not what happened
    # to this signal: a rename shows up in them only because the new name is
    # still nearby, and the same rename with the symbol carried out of the file
    # reads exactly like a deletion again. Letting an incidental neighbour count
    # as discrimination would report this promise as kept on an accident.

    def signals(self):
        """Every signal this instance watches."""
        body = self.read().body or []
        views = body if isinstance(body, list) else [body]
        return {v["key"] for v in views if "key" in v}

    def seen_keys(self):
        """Every signal this instance has actually looked at at least once."""
        body = self.read().body or []
        views = body if isinstance(body, list) else [body]
        return {v["key"] for v in views if (v.get("sightings") or 0) > 0}

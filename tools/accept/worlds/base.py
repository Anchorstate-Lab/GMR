"""A world is one kind of signal source, described without naming its domain.

This vocabulary is the single strongest guard on what GMR is. A scenario table
written in `function`, `signature`, `AST` would make this gate the most
persuasive evidence in the repository that GMR is a coding tool — prose in a
doc drifts, but a suite that runs every commit does not. So the events below
name what happens to *any* signal source, and a domain appears only as one
instantiation of them.

The rule that keeps it that way: **a scenario that can only be expressed in one
domain's words does not belong in the spec.** `matrix.py` enforces the other
half — that at least one registered world is not the coding one.
"""

import abc

# Every signal source can do these. A world that cannot express one of them is
# not a signal source, and `matrix.py` refuses to register it.
UNIVERSAL = (
    "reading_changed",
    "noise",
    "ceased",
)

# Only signal sources that have an identity and a place can do these. A reading
# taken from a deploy endpoint has neither, and saying so is honest; silently
# skipping would let the coding world quietly become the standard.
SHAPED = (
    "identity_changed",
    "location_changed",
    "neighborhood_changed",
)

EVENTS = UNIVERSAL + SHAPED


class World(abc.ABC):
    """One signal source, its fixture, and the events that can happen to it."""

    name = None
    expresses = ()

    # Whether this world's signal is derived from the source tree. `matrix.py`
    # refuses a suite in which every world answers True: a matrix made only of
    # parse trees would be the most persuasive artefact in this repository for
    # the claim that GMR is a coding tool, and it would run on every commit.
    derives_from_source = True

    # Whether readings from this signal carry named axes a memory can subscribe
    # to. A deploy sha has a value, not a vector, and says so rather than
    # pretending the question applies.
    has_axes = True

    # Whether this world's instrument can be swapped without rebuilding the
    # binary. Only a probe declared as a recipe can; a built-in extractor's
    # identity moves when the binary does.
    swappable_instrument = False

    @abc.abstractmethod
    def build(self, repo):
        """Lay the fixture down. Called before `gmr init`."""

    def recipes(self, repo):
        """Lay down whatever recipe files this world's probe needs.

        Separate from `declare` because a migrated instance needs the recipe but
        must not re-declare: what it watches arrives in the import, not from a
        second pass over the fixture.
        """

    def declare(self, gmr, repo):
        """Make the runtime watch this signal. Called after `gmr init`."""
        self.recipes(repo)
        gmr.declare(self.coordinate)

    @property
    @abc.abstractmethod
    def coordinate(self):
        """What a person names when they point at this signal."""

    @property
    def signal(self):
        """The key the runtime files it under. Usually the coordinate itself."""
        return self.coordinate

    def can(self, event):
        return event in self.expresses

    # ── the events ──────────────────────────────────────────────────────────

    @abc.abstractmethod
    def reading_changed(self, repo):
        """The fact this signal reports is now a different fact."""

    @abc.abstractmethod
    def noise(self, repo):
        """The representation changed; the fact did not."""

    @abc.abstractmethod
    def ceased(self, repo):
        """The signal is no longer there to read."""

    def identity_changed(self, repo):
        raise NotImplementedError

    def location_changed(self, repo):
        raise NotImplementedError

    def neighborhood_changed(self, repo):
        raise NotImplementedError

    def many(self, repo, n):
        """Lay down `n` signals of this kind and return their keys.

        Only the liveness scenario needs this, and it needs it in every world:
        a runtime that starves one kind of signal under a tight budget starves
        it whatever the domain.
        """
        raise NotImplementedError

    def declare_many(self, gmr, repo, n):
        """Lay down `n` more signals and put every one of them under watch."""
        keys = self.many(repo, n)
        if self.coordinate is None:
            gmr.declare()
        else:
            for k in keys:
                gmr.declare(k)
        return keys

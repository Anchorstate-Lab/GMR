import json
import os
import subprocess
import tempfile
import unittest

import gmr


def a_repository():
    root = tempfile.mkdtemp()
    subprocess.run(["git", "init", "-q", root], check=True)
    os.makedirs(os.path.join(root, "envs"), exist_ok=True)
    os.makedirs(os.path.join(root, "memories"), exist_ok=True)
    with open(os.path.join(root, "envs", "prod.yaml"), "w") as f:
        f.write("service:\n  replicas: 9\n")
    with open(os.path.join(root, "memories", "replicas.md"), "w") as f:
        f.write("Nine, because eight cannot survive a rolling restart.\n")
    return root


def opened(root):
    return gmr.open(
        {
            "root": root,
            "providers": {"git": True},
            "recipes": {
                "file": {
                    "replicas": {"path": "envs/{env}.yaml", "select": "$.service.replicas"}
                }
            },
        }
    )


def replicated(g):
    g.open(
        {
            "key": "prod-replicas",
            "probe": {"kind": "file", "name": "replicas"},
            "initial": {"position": {"env": "prod"}},
            "transitions": [
                {"when": "true", "to": "{ position: state.position, v: obs.value }"}
            ],
        }
    )


class Verbs(unittest.TestCase):
    def test_the_module_names_the_contract_it_serves(self):
        self.assertEqual(gmr.CONTRACT, "gmr.contract.v11")

    def test_five_lines_get_a_sentences_grounding(self):
        g = opened(a_repository())
        replicated(g)
        g.bind("git:memories/replicas.md", ["prod-replicas"], "derived")

        standing = g.ground(["git:memories/replicas.md"], {"max_staleness_ms": 0})[0]
        self.assertEqual(standing["claim"]["provider"], "git")
        self.assertEqual(len(standing["on"]), 1, "the sentence is about one anchor")
        self.assertEqual(standing["on"][0]["anchored"], "on")
        self.assertIn(
            standing["on"][0]["warrant"]["holding"]["holding"],
            ["holds", "moved", "incomparable", "absent", "never_established", "undated"],
        )

    def test_an_address_that_names_no_store_is_refused_before_anything_is_asked(self):
        g = opened(a_repository())
        with self.assertRaises(gmr.Fault) as refused:
            g.ground(["memories/replicas.md"])
        spoken = str(refused.exception)
        self.assertTrue(
            spoken.startswith("refused: "),
            f"the kind is a token in front of the prose: {spoken}",
        )
        self.assertIn("names nothing", spoken)

    def test_read_hands_back_the_envelope_and_a_carried_edge_says_who_asserted_it(self):
        g = opened(a_repository())
        replicated(g)
        g.bind("git:memories/replicas.md", ["prod-replicas"], "derived")
        g.link("git:memories/replicas.md", "git:memories/why.md", "rests-on", "adjudicated")

        view = g.read("prod-replicas")
        self.assertEqual(view["key"], "prod-replicas")
        self.assertEqual(len(view["memories"]), 1, "carry is opt-in")
        held = view["memories"][0]
        self.assertTrue(held["grounded"])
        self.assertEqual(len(held["links"]), 1)
        edge = held["links"][0]
        self.assertEqual(edge["to"], {"provider": "git", "external_id": "memories/why.md"})
        self.assertEqual(edge["kind"], "rests-on")
        self.assertEqual(edge["source"], "adjudicated")
        self.assertIsNotNone(edge.get("at"), "when the edge grew travels with it")

        dropped = g.unlink(
            from_="git:memories/replicas.md",
            to="git:memories/why.md",
            kind="rests-on",
            source="adjudicated",
        )
        self.assertEqual(dropped, 1)
        after = g.read("prod-replicas")
        self.assertEqual(after["memories"][0]["links"], [])

    def test_what_changed_since_a_cursor_comes_back_as_edges(self):
        g = opened(a_repository())
        replicated(g)
        seen = g.since(0)
        self.assertIsInstance(seen["edges"], list)




class Walk(unittest.TestCase):
    def test_the_walk_reaches_both_ends_and_a_conclusion_condenses(self):
        root = a_repository()
        g = opened(root)
        replicated(g)
        g.bind("git:memories/replicas.md", ["prod-replicas"], "derived")
        g.bind("said:s1", ["prod-replicas"], "self_attested")

        view = g.read("prod-replicas")
        self.assertEqual([s["id"] for s in view["said"]], ["s1"])
        self.assertIn("warrant", view["said"][0])

        self.assertEqual([a["key"] for a in g.anchors()], ["prod-replicas"])
        besides = g.cobound("git:memories/replicas.md")
        self.assertTrue(
            any(c == {"said": "s1"} for c in besides),
            f"cobound serves the utterance too: {besides}",
        )

        g.link("git:memories/replicas.md", "git:memories/why.md", "rests-on", "adjudicated")
        both = g.links("git:memories/why.md")
        self.assertEqual(len(both["in"]), 1)
        self.assertEqual(both["in"][0]["from"]["external_id"], "memories/replicas.md")
        self.assertIsNotNone(both["in"][0].get("at"))
        self.assertEqual(both["out"], [])

        lean = g.read("prod-replicas", {"lean": True})
        self.assertNotIn("content", lean["memories"][0]["grounding"])

        with open(os.path.join(root, "memories", "conclusion.md"), "w") as f:
            f.write("Grown into a memory.\n")
        g.condense("said:s1", "git:memories/conclusion.md", "derived")
        after = g.read("prod-replicas")
        self.assertNotIn("said", after)
        held = [
            m for m in after["memories"]
            if m["reference"]["external_id"] == "memories/conclusion.md"
        ]
        self.assertEqual(held[0]["origin"], "s1")


if __name__ == "__main__":
    unittest.main(verbosity=1)

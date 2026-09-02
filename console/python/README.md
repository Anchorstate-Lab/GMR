# gmr — the Python door

The same console verb table the node binding serves, synchronous, with
values as dicts in and out. `import gmr` after installing `anchorstate-gmr`.

```python
import gmr

g = gmr.open({"root": ".", "providers": {"git": True}})
reading = g.sample("prod-replicas", {"max_staleness_ms": 60000})
g.bind("said:t1", ["prod-replicas"], "self_attested",
       {"saw": [reading["fact_address"]], "depends": "all(anchors, not state.v.value)"})
standing = g.ground(["said:t1"])
```

One look at the world, cited — see the repository's `examples/menu` for the
whole story with no code in sight.

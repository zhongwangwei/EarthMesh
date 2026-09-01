# Alpha6 CEC proof, checkpoint, and cache contract

PR92 extends the rollback CEC solver with a finite proof mode. Unlike PR91
find-one, this mode may emit `ExactNoSolution`, but only after every pending
proof shard is exhausted and every reached downstream plan is exactly rejected.
Any cycle-search budget stop produces a typed checkpoint; any downstream
incomplete or invalid result remains `DownstreamSearchIncomplete`.

## Proof shards and resume

A `CycleProofShard` binds a canonical decision prefix to the complete
`EssentialCycleProblemKey`. Prefixes contain canonical edge IDs, never runtime
slots. At a unique-state budget boundary the DFS records the current subtree
and each not-yet-entered sibling. Resumption replays the prefix from the root,
runs the same degree propagation, and continues the rollback search.

`merge_cycle_search_checkpoints` validates the full problem key, removes exact
duplicate shards, and orders prefixes with the solver's include-first branch
order. This is the deterministic merge surface for independently executed
chunks; thread completion order cannot change the resumed search order.

The core crate returns typed checkpoints only. JSONL/binary persistence,
`.partial` files, and atomic rename remain CLI responsibilities, so no
serialization dependency was added.

## Downstream cache

Cache equality uses:

```text
(complete EssentialCycleProblemKey, canonical EssentialCycleKey,
 full-polygon topology-state budget)
```

The complete problem key already includes the essential-cycle and downstream
contract versions. Accepted, exact-rejected, incomplete, and invalid values are
stored as distinct typed results. Reusing an incomplete value therefore cannot
turn it into a no-go.

## Closed small fixture

The N12 Interior-Control legacy W2 family is a small exhaustible fixture. Its
CEC candidate graph has 6 vertices and 3 edges. Proof mode closes it in one
unique propagated state: all three candidates are forced off and the resulting
permanently open coarse-to-fine dual path proves that no essential cycle exists
in this declared problem. No downstream evaluator is invoked.

Checkpoint regression uses the known Frozen N6 F0 family. A one-state chunk
creates a frontier, resumption returns the same canonical cycle and
`FaceBandPlan` as one-shot execution, duplicate checkpoint merges are stable,
and the accepted downstream result is subsequently served by the exact-keyed
cache.

These are scoped finite-family conclusions. PR92 does not reclassify the 659
Alpha5 unknowns and does not change a product or validation gate.

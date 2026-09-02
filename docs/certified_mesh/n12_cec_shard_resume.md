# Lifted-N12 CEC shard resume

PR121 resumes every one of the 49 CEC checkpoints left by the original
16,384-state N12 search. Each shard receives an independent 256-state quantum
and the same bounded SDCE screening used by PR117: eight annular seeds per
cell, beam width one, no flips, and no joint-pair extraction.

| Result | Count |
| --- | ---: |
| Input shards resumed | 49 |
| Exact no-solution shards | 1 |
| CEC-complete / downstream-incomplete shards | 3 |
| Still CEC-incomplete shards | 45 |
| Invalid shards | 0 |
| Resumed unique states | 11,597 |
| Essential cycles screened | 3,937 |
| SDCE pairs scored | 251,968 |
| Best incidence distance | 16 |

The 45 incomplete searches split into 1,233 deterministic continuation
checkpoints. This is frontier refinement, not 1,233 newly discovered failures:
each old depth-first continuation was advanced and divided at its new bounded
stopping points. The resume is therefore not complete.

No new topology closed in this shallow screening quantum. Geometry was not
run, the failed PR118 strict-geometry gate remains unchanged, and N24, N40,
and NXP80 stay locked.

Frozen evidence:

- `rust/earthmesh_refine_certified/tests/fixtures/n12_cec_shard_resume.json`

Taskbook SHA-256:
`65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473`.

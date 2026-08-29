# Frozen real-IGBP reference

The current branch contains the latest auditable real-IGBP NXP80 four-artifact
record in `docs/harp_window_budget_ab_evidence.md` (code `4b5feeeb`). The task
plan refers to a later PR #23 baseline, but that merge and its artifacts are not
present in this repository history; this file therefore records the available
frozen evidence without mislabelling it.

| artifact | SHA-256 |
|---|---|
| gridinit | `023622dab86e12929e06359730905fb04b1d4b96d8efe2c72b144d48646c864a` |
| raw refined grid | `6d5fd69aee11fa031c6a21ce15972b6e8ef63a36d416b5fcc691aa8423180cdc` |
| final gridfile | `5f93bb854fc9497b00f2a4eaa087255fc08ea475ec28afec95bfb4e7d1c0dd97` |
| conservative remap | `0041a1058b5e4c7a2eb3b1c17f88fe793455aafefe5d590849c3ec312aec5e78` |

CMRC acceptance uses the same real input class but does not require byte parity
with another algorithm. These hashes protect the existing-backend baseline
from accidental changes while CMRC is added as a peer option.

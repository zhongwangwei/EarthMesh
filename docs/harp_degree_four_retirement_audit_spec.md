# HARP degree-four retirement audit micro-spec (PR20b)

## 1. Purpose and boundary

PR20b answers one question without changing mesh decisions:

> At the production leaf-retirement checkpoint, which degree-four sites are eligible, which of their two retriangulation trials are feasible, and which independent acceptance checks reject each trial?

It does **not** add a retirement rule, change candidate order or the 64-site cap, add a two-ring patch/cavity search, or claim to solve the final 28,161 angle-window violations. PR20a already showed that 99.04% of that residual is not forced by degree <=4.

The HARP crate owns typed audit semantics and remains independent of `serde`/`serde_json`. The CLI alone owns JSONL and filesystem policy.

## 2. Activation and observation point

The audit remains opt-in through:

```text
EARTHMESH_HARP_D4_RETIREMENT_AUDIT
```

It runs only on meshes without protected segments. It reads the mesh immediately before the existing production leaf-retirement sweep, uses that sweep's real candidate ranking, and never mutates the source mesh. After the unchanged production sweep completes, committed degree-four sites are joined back to the audit by stable `SiteId`.

When `EARTHMESH_HARP_TRACE_JSONL` is also enabled, the typed audit records are written into the same fail-closed trace before the S6 `final` summary. A trace write or publication failure keeps the existing PR20a rule: no final gridfile or conservative-remap delivery.

## 3. Site populations

All site counts use a site denominator, never a trial denominator:

- `sites_total`: all active degree-four vertices at the audit checkpoint.
- `sites_not_leaf`: `sites_total` sites that are not active interior leaves.
- `sites_eligible`: active degree-four interior leaves.
- `sites_without_window_violation`: eligible sites absent from the real window-breaching retirement candidate list.
- `sites_audited`: eligible sites present in that candidate list.
- `sites_ranked_beyond_64`: audited sites whose position in the real mixed degree `3..=maximum_retirement_degree` ranking is outside the existing 64-site production cap.
- `sites_with_any_valid_trial`: audited sites with at least one geometry-valid trial.
- `sites_with_any_fully_acceptable_trial`: audited sites with at least one trial passing every required check.
- `sites_committed`: audited sites actually committed by the unchanged production sweep.

Required closure:

```text
sites_total = sites_not_leaf + sites_eligible
sites_eligible = sites_without_window_violation + sites_audited
sites_committed <= sites_with_any_fully_acceptable_trial <= sites_with_any_valid_trial <= sites_audited
```

A violation of these equations is an audit error, not evidence to publish.

## 4. Trial identity and denominator

Every audited degree-four site has exactly two diagonal trials, including geometrically invalid alternatives. A trial has stable identity:

```text
(site_id, trial_index)
```

and carries:

- the four stable ring `SiteId`s;
- the two stable diagonal endpoint `SiteId`s when the ring is measurable;
- `trial_index` in deterministic diagonal-key order.

Records are emitted in ascending `site_id`, then ascending `trial_index` order.

Required closure:

```text
trials_total = 2 * sites_audited
```

## 5. Three-state checks

Each check is exactly one of:

```text
pass
fail
not_evaluated
```

`not_evaluated` means the check had no valid input or could not be measured. It must never be aggregated as `fail`.

The checks are:

1. `geometry`: the real retirement candidate can be built, remains closed and valid, and passes local Delaunay legalization.
2. `hard_gate`: the existing HARP hard-gate checker accepts the candidate.
3. `physical_demand`: physical demand count is measurable and does not increase.
4. `scale_balance`: unbalanced-pair count does not increase, and worst excess does not increase while imbalance remains.
5. `no_new_low_degree`: the post-trial degree-<5 set is a strict subset of the pre-trial set for a degree-four retirement.
6. `angle_count`: angles below 40 degrees and above 80 degrees do not increase individually, their total strictly decreases, and no angle is unmeasurable.
7. `worst_deviation`: the worst 40..80-degree window deviation does not increase.
8. `penalty`: the 40..80-degree window penalty strictly decreases.
9. `eta`: the global minimum eta is measurable and does not decrease.
10. `margin`: the global minimum window margin is measurable and does not decrease.
11. `conservative_remap`: the same production conservative-remap construction succeeds.

For a `geometry=fail` trial, every later check is `not_evaluated`. For a geometry-valid trial, independent measurable checks are evaluated even if another check fails; only unavailable measurements are `not_evaluated`.

A trial is `fully_acceptable` only when all eleven checks are `pass`.

## 6. Records and summaries

The typed core trace adds three semantic record kinds:

- one degree-four audit summary;
- one site row for every `sites_total` site;
- two trial rows for every `sites_audited` site.

The summary carries site counts, `trials_total`, and per-check `pass/fail/not_evaluated` counts. For every check:

```text
pass + fail + not_evaluated = trials_total
```

The HARP run report must explicitly state whether the audit was evaluated. Zero counts with `evaluated=false` mean "not run", not "run and found nothing".

## 7. Acceptance criteria

PR20b is acceptable only if:

1. trace-off output remains byte-identical to the PR20a baseline;
2. enabling the audit and trace does not change the final mesh, gridfile, or conservative remap;
3. site and trial closure equations hold;
4. every trial/check has one three-state value and `not_evaluated` remains distinct from `fail`;
5. repeated identical runs emit deterministic audit records;
6. audit trace failure remains fail-closed before final delivery;
7. core tests, CLI tests, lint, and the real IGBP NXP80 audit run pass.

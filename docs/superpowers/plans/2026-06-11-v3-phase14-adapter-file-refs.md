# EarthMesh v3 Phase 14 Adapter File Reference Sidecars Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Add file references to adapter export sidecars so downstream CoLM/MPAS/FVCOM writers can discover the manifest and overlay QA sidecars associated with each adapter plan.

**Architecture:** Keep `AdapterExportPlan` immutable. When writing pipeline sidecars, create a new plan instance with a populated `files` map using `dataclasses.replace()` and write that to disk without mutating the in-memory plan.

**Tech Stack:** Python standard library, pytest. No new dependencies.

---

## Task

- [ ] Add failing test that adapter JSON includes `files.manifest` and `files.overlay_summary`.
- [ ] Implement file-reference injection during `V3PipelineResult.write_sidecars()`.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.

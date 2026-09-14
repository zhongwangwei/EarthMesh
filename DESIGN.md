# Design

## Source of truth
- Active, refreshed 2026-09-14. Scope: EarthMesh Studio desktop GUI.
- Evidence: `gui-tauri/dist/index.html`, `gui-tauri/README.md`, `scripts/check_gui_map_e2e.py`, `scripts/check_gui_js.js`.

## Brand
Scientific, compact, evidence-led. Show actual quality/delivery state; do not imply solver certification.

## Product goals
Configure demand → select algorithm → inspect global/regional quality → deliver model files. Land/atmosphere/ocean differ through configuration, not separate UI workflows or numerical implementations.

## Personas and jobs
Earth-system researchers create, reopen, edit, run and inspect reproducible Project YAMLs.

## Information architecture
Keep the seven-step workflow, left navigation, center editor and right map/quality/log panes. Domain geometry belongs in step 3; refinement geometry is separate.

## Design principles
Reuse existing controls. Preserve unedited project fields. Surface unsupported capabilities and invalid input rather than silently changing the request.

## Visual language
Reuse the existing light/dark CSS tokens, system fonts, spacing, radii and map styles in `dist/index.html`. No new theme or visual assets.

## Components
Use native buttons and numeric inputs inside existing cards and two-column fields. Circle domain adds center longitude/latitude and geodesic radius in km alongside existing domain modes and sea-ratio control.

## Accessibility
Use named inputs, keyboard-operable mode buttons, visible focus and selected state. Label units and invalid input; never turn a blank numeric field into zero.

## Responsive behavior
Keep existing desktop split panes and wrapping controls. Verify at 1400 px and 1000 px widths in both languages; no new mobile layout.

## Interaction states
Each domain mode retains its geometry and full-precision sea ratio within a project; opening a different project resets its alternate drafts. Preview and picker responses belong to the current domain, never a superseded request. Valid edits update map and estimate; invalid edits show an error and cannot save/run as an earlier valid request. Clear obsolete run results after domain edits. Opening and language/step changes retain values. Boundary previews are not delivered meshes. Open/recent/New are latest-request-owned; obsolete reads, validation, summary or save callbacks cannot repaint a newer project. Specified geometry drafts stay editable even when invalid and cannot silently become defaults; inactive drafts do not affect another source. Multi-circle chains are preserved with a read-only head, not partially editable.

## Content voice
Concise Chinese/English scientific terms. State geodesic radius and the minor-hemisphere limit; distinguish domain circle from refinement circle. Explain existing polar map-display limits without restricting the computational domain.

## Implementation constraints
Static HTML/JS + existing Tauri commands; shared Rust Project validation is authoritative. Use vendored OpenLayers spherical geometry and existing MapLibre rendering. No new dependencies or polygon schema. Preserve old unexposed-shape fallback.

## Open questions
None blocking this batch. Arbitrary polygon domain editing requires separate schema support; not part of circle-domain completion.

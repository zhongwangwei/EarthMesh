# MERIT Delivery Package Bridge Design

**Goal:** Connect MERIT-Hydro 90m mask outputs to the existing hydro/coast delivery package boundary without replacing the package, HTML, or CoLM coupling code paths.

**Requirements:**
- Read MERIT-Hydro tiles through the existing `write_merit_mask_outputs()` bridge.
- Project MERIT `R2/R3` masks onto background EarthMesh cells as river intersection GeoJSON.
- Project MERIT `COAST_LAND/COAST_OCEAN` masks onto background EarthMesh cells as coast intersection GeoJSON with `coastal_fraction`.
- Reuse `write_refinement_delivery_package()` so outputs keep the same manifest, complete cell mask, HTML map, and CoLM coupling compatibility.
- Write a bridge summary containing source masks, derived intersection files, thresholds, bbox, stride, and feature counts.
- Keep the workflow safe for large 90m data: users can choose bbox/stride and should smoke-test small windows before full N112/China runs.

**Validation:**
- Unit test with a tiny MERIT NetCDF fixture proves river/coast/surface/package/CoLM outputs are wired together.
- Real local smoke on `/Volumes/Data01/MERIT_Hydro` proves a 0.2 degree GBA window can produce a package, HTML, complete cell mask, and CoLM summary.

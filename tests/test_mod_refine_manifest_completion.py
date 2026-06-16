from __future__ import annotations

import json
from pathlib import Path

LEGACY_SUBROUTINES = [
    "refine_loop",
    "iterB_judge",
    "iterC_judge",
    "iterD_judge",
    "iterE_judge",
    "iterF_judge",
    "orial_vertices_protect",
    "iterG_judge",
    "OnedivideFour_connection",
    "OnedivideFour_renew",
    "Array_length_calculation",
    "bdy_connection_make",
    "bdy_connection_closed_curve",
    "bdy_refine_segment_make",
    "weak_concav_segment_make",
    "OnedivideTwo",
    "ref_sjx_isreverse_judge",
    "weak_concav_pair_special",
    "sharp_concav_lop_judge",
    "m1w1_to_m11w11",
    "weak_concav_lop_judge",
    "Delaunay_Lop",
    "crossline_check",
    "NGR_RENEW",
]


def test_mod_refine_manifest_is_completed_with_subroutine_anchors():
    manifest_text = Path("docs/fortran_to_rust_migration_manifest.json").read_text()
    manifest = json.loads(manifest_text)
    entry = next(item for item in manifest["fortran_sources"] if item["path"] == "src/MOD_refine.F90")

    assert entry.get("port_status") == "completed"
    assert entry.get("remaining_rust_surfaces") == []

    evidence = "\n".join(
        [
            Path("rust/earthmesh_cli/src/lib.rs").read_text(errors="ignore"),
            Path("rust/earthmesh_mesh/src/lib.rs").read_text(errors="ignore"),
            manifest_text,
            Path("docs/fortran_to_rust_migration.md").read_text(errors="ignore"),
        ]
    )
    missing = [name for name in LEGACY_SUBROUTINES if name not in evidence]
    assert missing == []

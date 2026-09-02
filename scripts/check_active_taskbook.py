#!/usr/bin/env python3
"""Fail closed if the active taskbook or frozen Alpha6 evidence drifts."""

from __future__ import annotations

import hashlib
from pathlib import Path
import sys
import tomllib


EXPECTED = {
    "schema_version": 1,
    "taskbook_id": "CMRC-DQX-2026-09-03-R1",
    "taskbook_sha256": "a8650d0c882330fb7ef6ba35945192b8943ceb4fba8aa60d2c8d67c169a17951",
    "source_file_sha256": "1e5025740cf826bfc3f64c01a2faa6ecca37baaf92ba03b8c8e0fdeb233bc0db",
    "baseline_branch": "v3.0.0-alpha6",
    "baseline_commit": "20551dbd24ec05556bbe9a4e7f913803cf77b001",
    "implementation_branch": "v3.0.0-alpha7",
    "quality_policy": "domain_export",
}
RESEARCH_ONLY = {
    "CEC",
    "SDCE",
    "general_annular_topology_search",
    "anchor_ear_portfolio",
    "unbounded_untangle",
}


def main() -> int:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
    manifest = root / "docs/certified_mesh/ACTIVE_TASKBOOK.toml"
    data = tomllib.loads(manifest.read_text())

    for key, expected in EXPECTED.items():
        if data.get(key) != expected:
            raise SystemExit(f"{manifest}: {key} must be {expected!r}")
    if set(data.get("research_only_paths", ())) != RESEARCH_ONLY:
        raise SystemExit(f"{manifest}: research_only_paths drifted")

    evidence = data.get("frozen_evidence", ())
    if not evidence:
        raise SystemExit(f"{manifest}: frozen_evidence is empty")
    for item in evidence:
        path = root / item["path"]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if item.get("research_only") is not True or digest != item["sha256"]:
            raise SystemExit(f"{path}: frozen research evidence drifted")

    print(f"active taskbook {data['taskbook_id']} and {len(evidence)} frozen evidence files verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

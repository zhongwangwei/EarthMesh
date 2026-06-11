from __future__ import annotations

import gzip
import json
from pathlib import Path
from typing import Any


def read_json(path: str | Path) -> Any:
    input_path = Path(path)
    if input_path.suffix == ".gz":
        with gzip.open(input_path, "rt") as handle:
            return json.load(handle)
    return json.loads(input_path.read_text())


def write_json(path: str | Path, payload: Any) -> Path:
    output_path = Path(path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    if output_path.suffix == ".gz":
        with gzip.open(output_path, "wt") as handle:
            json.dump(payload, handle, indent=2, sort_keys=True)
            handle.write("\n")
    else:
        output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return output_path

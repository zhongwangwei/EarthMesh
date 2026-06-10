import json
import struct

from util.hydro_mesh.cama_sample import sample_cama_window_to_jsonl


def test_sample_cama_window_to_jsonl_writes_classified_records(tmp_path):
    map_dir = tmp_path / "map"
    map_dir.mkdir()
    (map_dir / "params.txt").write_text(
        "           3      !! grid number (east-west)\n"
        "           2      !! grid number (north-south)\n"
        "          10     !! floodplain layer\n"
        "   1.0000000     !! grid size\n"
        "       0.000     !! west  edge (deg)\n"
        "       3.000     !! east  edge (deg)\n"
        "       0.000     !! south edge (deg)\n"
        "       2.000     !! north edge (deg)\n"
    )
    (map_dir / "uparea.bin").write_bytes(struct.pack("<6f", 0.0, 2_000_000_000.0, 0.0, 20_000_000_000.0, 0.0, 0.0))
    (map_dir / "width.bin").write_bytes(struct.pack("<6f", 0.0, 80.0, 0.0, 3000.0, 0.0, 0.0))
    (map_dir / "rivlen.bin").write_bytes(struct.pack("<6f", 0.0, 1000.0, 0.0, 1500.0, 0.0, 0.0))
    (map_dir / "nextxy.bin").write_bytes(
        struct.pack(
            "<12i",
            0, 0,
            2, 0,
            0, 0,
            1, 1,
            0, 0,
            0, 0,
        )
    )
    output = tmp_path / "classified.jsonl"

    records = sample_cama_window_to_jsonl(
        map_dir,
        output,
        bbox=(0.0, 0.0, 3.0, 2.0),
        target_dx_km=10.0,
        uparea_to_km2=1e-6,
        y_reversed_storage=False,
    )

    assert [record["river_class"] for record in records] == ["R1", "R3"]
    written = [json.loads(line) for line in output.read_text().splitlines()]
    assert written[0]["reach_id"] == "cama-0-1"
    assert written[0]["upstream_area_km2"] == 2000.0
    assert written[0]["lon"] == 1.5
    assert written[0]["lat"] == 0.5

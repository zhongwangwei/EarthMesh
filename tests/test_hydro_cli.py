import csv
import json

from util.hydro_mesh.cli import classify_csv


def test_classify_csv_writes_json_lines(tmp_path):
    input_csv = tmp_path / "reaches.csv"
    output_jsonl = tmp_path / "classified.jsonl"

    with input_csv.open("w", newline="") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=[
                "reach_id",
                "upstream_area_km2",
                "width_m",
                "floodplain_width_m",
                "target_dx_km",
                "is_estuary",
            ],
        )
        writer.writeheader()
        writer.writerow(
            {
                "reach_id": "outlet",
                "upstream_area_km2": "800",
                "width_m": "50",
                "floodplain_width_m": "100",
                "target_dx_km": "10",
                "is_estuary": "true",
            }
        )
        writer.writerow(
            {
                "reach_id": "small-channel",
                "upstream_area_km2": "2000",
                "width_m": "50",
                "floodplain_width_m": "80",
                "target_dx_km": "10",
                "is_estuary": "false",
            }
        )

    records = classify_csv(input_csv, output_jsonl)

    assert [record["river_class"] for record in records] == ["R3", "R1"]
    assert output_jsonl.exists()
    written = [json.loads(line) for line in output_jsonl.read_text().splitlines()]
    assert [record["reach_id"] for record in written] == ["outlet", "small-channel"]
    assert written[0]["reasons"] == ["estuary"]


def test_classify_csv_accepts_boolean_variants(tmp_path):
    input_csv = tmp_path / "reaches.csv"
    output_jsonl = tmp_path / "classified.jsonl"
    input_csv.write_text(
        "reach_id,upstream_area_km2,width_m,floodplain_width_m,target_dx_km,is_delta\n"
        "delta-node,100,20,20,10,yes\n"
    )

    records = classify_csv(input_csv, output_jsonl)

    assert records[0]["river_class"] == "R3"
    assert records[0]["reasons"] == ["delta"]

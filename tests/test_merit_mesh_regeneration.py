import json
from pathlib import Path

import netCDF4
import numpy as np


def test_merit_mesh_regeneration_inputs_write_close_masks_and_patched_namelist(tmp_path):
    from util.hydro_mesh.merit_mesh_regeneration import write_merit_mesh_regeneration_inputs

    merit_root = tmp_path / "merit"
    merit_root.mkdir()
    _write_merit_fixture(merit_root / "n20e110.nc")
    template = tmp_path / "template.mnl"
    template.write_text(
        """
&mkgrd
  NL%EXPNME = 'old_case'
  NL%base_dir = '/old/base/'
/
&mkrefine
  RL%max_iter_spc = 1
  RL%mask_refine_spc_fprefix = '/old/refine_prefix'
/
""".strip()
        + "\n"
    )

    result = write_merit_mesh_regeneration_inputs(
        case_name="merit_regen_case",
        merit_root=merit_root,
        bbox=(110.0, 20.0, 110.005, 20.005),
        output_dir=tmp_path / "regen",
        template_nml=template,
        case_base_dir=tmp_path / "cases",
        stride=1,
        compress_raw_merit=True,
        r2_cap=4,
        r3_cap=4,
        coast_cap=4,
    )

    assert result["raw_merit"]["river_masks"].name == "merit_river_masks.geojson.gz"
    assert result["raw_merit"]["coast_masks"].name == "merit_coast_masks.geojson.gz"
    assert result["close_mask_summary"].counts_by_component["merit_river"] > 0
    assert result["close_mask_summary"].counts_by_component["merit_coast"] > 0
    assert result["patched_nml"].exists()
    patched = result["patched_nml"].read_text()
    assert "NL%EXPNME = 'merit_regen_case'" in patched
    assert f"NL%base_dir = '{tmp_path / 'cases'}/'" in patched
    assert f"RL%mask_refine_spc_fprefix = '{result['close_mask_prefix']}'" in patched
    assert "RL%max_iter_spc = 3" in patched
    summary = json.loads(result["summary_json"].read_text())
    assert summary["kind"] == "earthmesh_merit_mesh_regeneration_inputs"
    assert summary["files"]["patched_nml"] == str(result["patched_nml"])


def _write_merit_fixture(path: Path) -> None:
    with netCDF4.Dataset(path, "w") as ds:
        ds.createDimension("longitude", 6)
        ds.createDimension("latitude", 6)
        lon = ds.createVariable("longitude", "f8", ("longitude",))
        lat = ds.createVariable("latitude", "f8", ("latitude",))
        lon[:] = np.array([110.0, 110.001, 110.002, 110.003, 110.004, 110.005])
        lat[:] = np.array([20.005, 20.004, 20.003, 20.002, 20.001, 20.0])
        for name, dtype in [("dir", "i1"), ("upa", "f4"), ("elv", "f4"), ("wth", "f4"), ("landtype_igbp", "i1")]:
            ds.createVariable(name, dtype, ("longitude", "latitude"))
        ds.variables["dir"][:, :] = 1
        ds.variables["upa"][:, :] = np.array(
            [
                [0, 0, 0, 0, 0, 0],
                [0, 1000, 6000, 1000, 0, 0],
                [0, 2000, 60000, 2000, 0, 0],
                [0, 1000, 6000, 1000, 0, 0],
                [0, 0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0, 0],
            ],
            dtype="f4",
        )
        ds.variables["wth"][:, :] = np.array(
            [
                [0, 0, 0, 0, 0, 0],
                [0, 10, 80, 10, 0, 0],
                [0, 20, 350, 20, 0, 0],
                [0, 10, 80, 10, 0, 0],
                [0, 0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0, 0],
            ],
            dtype="f4",
        )
        ds.variables["elv"][:, :] = 1.0
        landtype = np.ones((6, 6), dtype="i1")
        landtype[3:, :] = 17
        ds.variables["landtype_igbp"][:, :] = landtype

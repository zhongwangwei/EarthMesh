#!/usr/bin/env python3
"""Small independent auditor regression; python scripts/test_colm_mesh_smoke.py."""
from pathlib import Path
import struct
import tempfile
import sys

import netCDF4
import numpy as np

from run_colm_mesh_smoke import audit, run


def main():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        source = root / 'input.nc'
        landdata = root / 'landdata'
        for path in ('block.nc', 'mesh/2005/mesh.nc', 'landelm/2005/landelm_w000_n00.nc'):
            file = landdata / path
            file.parent.mkdir(parents=True, exist_ok=True)
            file.touch()
        # Deliberately nonsquare with reversed latitude and unrelated global IDs.
        with netCDF4.Dataset(source, 'w') as ds:
            ds.createDimension('nlon', 3)
            ds.createDimension('nlat', 2)
            ds.createDimension('cell', 2)
            for name, dim, values in [('lon_w', 'nlon', [0, 1, 2]), ('lon_e', 'nlon', [1, 2, 3]),
                                      ('lat_s', 'nlat', [1, 0]), ('lat_n', 'nlat', [2, 1]),
                                      ('cell_id', 'cell', [2, 9]), ('pixel_count', 'cell', [2, 3])]:
                ds.createVariable(name, 'f8' if name.startswith(('lat', 'lon')) else 'i8', (dim,))[:] = values
            ds.createVariable('elmindex', 'i4', ('nlat', 'nlon'))[:] = [[2, 9, 0], [2, 9, 9]]
        with netCDF4.Dataset(landdata / 'pixel.nc', 'w') as ds:
            # Extra block interval shifts the longitude indices by one.
            ds.createDimension('x', 4)
            ds.createDimension('y', 2)
            for name, dim, values in [('lon_w', 'x', [-1, 0, 1, 2]), ('lon_e', 'x', [0, 1, 2, 3]),
                                      ('lat_s', 'y', [0, 1]), ('lat_n', 'y', [1, 2])]:
                ds.createVariable(name, 'f8', (dim,))[:] = values
        prefix = root / 'build'
        dump = root / 'build.0.bin'

        def record(cell, x, y):
            return struct.pack('<qq', cell, len(x)) + np.asarray(x, dtype='<i4').tobytes() + np.asarray(y, dtype='<i4').tobytes()

        first = record(2, [2, 2], [1, 2])
        second = record(9, [3, 4, 3], [1, 1, 2])
        valid = b'EMCOLM01' + first + second
        dump.write_bytes(valid)
        assert audit(source, landdata, prefix) == {'cells': 2, 'pixels': 5, 'workers': 1, 'exact_input_membership': True}
        malformed = [b'BADMAGIC' + first + second, valid[:-1], valid + first,
                     b'EMCOLM01' + first, b'EMCOLM01' + first + record(9, [3, 3, 3], [1, 1, 2]),
                     b'EMCOLM01' + first + record(9, [3, 4, 3], [1, 2, 2]),
                     b'EMCOLM01' + first + record(9, [0, 4, 3], [1, 1, 2]),
                     b'EMCOLM01' + first + record(9, [1, 4, 3], [1, 1, 2]),
                     b'EMCOLM01' + first + record(9, [3, 4], [1, 1])]
        for bad in malformed:
            dump.write_bytes(bad)
            try:
                audit(source, landdata, prefix)
            except ValueError:
                pass
            else:
                raise AssertionError('corrupt mesh accepted')
        dump.write_bytes(valid)
        # No test can rely just on a successful model marker / nonempty dump.
        with netCDF4.Dataset(source, 'r+') as ds:
            ds['pixel_count'][0] = 3
        try:
            audit(source, landdata, prefix)
        except ValueError:
            pass
        else:
            raise AssertionError('wrong input counts accepted')
        for message in ('STOP 0', 'Netcdf error: invalid input', 'Fortran runtime error: invalid bound'):
            try:
                run([sys.executable, '-c', f'print({message!r}); print("OK")'], root,
                    root / 'fatal.log', [], 10, 'OK')
            except ValueError:
                pass
            else:
                raise AssertionError('zero-exit fatal log accepted')
        print('PASS: exact union-grid mapping, 10 corruptions and 3 zero-exit fatal logs rejected')


if __name__ == '__main__':
    main()

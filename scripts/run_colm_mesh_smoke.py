#!/usr/bin/env python3
"""Build an isolated CoLM mesh consumer; verify build/save/fresh-load against raster.

Requires existing numpy, netCDF4, mpifort, make and nf-config. This exercises
mesh/landelm only, not landpatch, surface aggregation or the CoLM solver.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import signal
import struct
import subprocess
import sys
import tarfile
import time

import netCDF4
import numpy as np

FLAGS = shlex.split('-fopenmp -O2 -fdefault-real-8 -ffree-form -g -fcheck=all '
                    '-ffpe-trap=invalid,zero,overflow -fbacktrace -cpp '
                    '-ffree-line-length-0 -fallow-argument-mismatch')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(path):
    with open(path, 'rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def axis_map(original, model, first, second):
    """Map CoLM union-grid edges, not index offsets, onto original pixel edges."""
    a = np.asarray(original[first][:])
    b = np.asarray(original[second][:])
    x = np.asarray(model[first][:])
    y = np.asarray(model[second][:])
    require(all(np.isfinite(v).all() for v in (a, b, x, y)), 'non-finite edges')
    order = np.argsort(a)
    pos = np.searchsorted(a[order], x)
    hi = order[np.minimum(pos, len(a) - 1)]
    lo = order[np.maximum(pos - 1, 0)]
    index = np.where(abs(a[hi] - x) < abs(a[lo] - x), hi, lo)
    matched = (abs(a[index] - x) < 1e-9) & (abs(b[index] - y) < 1e-9)
    return np.where(matched, index, -1)


def audit(mesh_path, landdata, prefix):
    """Every dumped membership must equal input, with no duplicate or missing pixel."""
    for file in ('block.nc', 'pixel.nc', 'mesh/2005/mesh.nc'):
        require((landdata / file).is_file(), f'missing saved {file}')
    require(any((landdata / 'landelm/2005').glob('landelm_*.nc')), 'missing saved landelm blocks')
    with netCDF4.Dataset(mesh_path) as source, netCDF4.Dataset(landdata / 'pixel.nc') as saved:
        source.set_auto_mask(False)
        saved.set_auto_mask(False)
        require(source['elmindex'].dimensions == ('nlat', 'nlon'), 'wrong elmindex dimension order')
        require(all(source[name].dtype.kind in 'iu' for name in ('elmindex', 'cell_id', 'pixel_count')),
                'raster and cell metadata must be integers')
        require(source['elmindex'].size <= 268435456, 'raster exceeds smoke memory limit')
        raster = np.asarray(source['elmindex'][:])
        ids = np.asarray(source['cell_id'][:], dtype=np.int64)
        counts = np.asarray(source['pixel_count'][:], dtype=np.int64)
        require(ids.ndim == counts.ndim == 1 and len(ids) == len(counts) > 0,
                'invalid cell metadata')
        require(np.all(ids > 0) and np.all(counts > 0) and len(np.unique(ids)) == len(ids),
                'nonpositive or duplicate cell metadata')
        expected = dict(zip(ids.tolist(), counts.tolist()))
        xmap = axis_map(source, saved, 'lon_w', 'lon_e')
        ymap = axis_map(source, saved, 'lat_s', 'lat_n')
    require(np.all(raster >= 0), 'negative raster ID')
    require(int(np.count_nonzero(raster)) == sum(expected.values()), 'input pixel_count mismatch')
    flat = raster.ravel()
    seen = np.zeros(flat.size, dtype=bool)
    found = set()
    dumps = sorted(prefix.parent.glob(prefix.name + '.*.bin'))
    require(bool(dumps), 'no model worker dumps')
    for dump in dumps:
        with dump.open('rb') as stream:
            require(stream.read(8) == b'EMCOLM01', 'bad worker dump magic')
            while header := stream.read(16):
                require(len(header) == 16, 'truncated record header')
                cell, count = struct.unpack('<qq', header)
                require(cell in expected and cell not in found, 'unknown or duplicate element')
                require(count == expected[cell], f'pixel count differs for element {cell}')
                ix = np.fromfile(stream, dtype='<i4', count=count)
                iy = np.fromfile(stream, dtype='<i4', count=count)
                require(len(ix) == len(iy) == count, 'truncated pixel arrays')
                require(np.all((ix > 0) & (ix <= len(xmap))) and
                        np.all((iy > 0) & (iy <= len(ymap))), 'model index outside pixel grid')
                x, y = xmap[ix - 1], ymap[iy - 1]
                require(np.all(x >= 0) and np.all(y >= 0), 'model pixel split or outside input grid')
                linear = y * raster.shape[1] + x
                require(len(np.unique(linear)) == count and not seen[linear].any(), 'duplicate pixel')
                require(np.all(flat[linear] == cell), f'ownership differs for element {cell}')
                seen[linear] = True
                found.add(cell)
    require(found == set(expected), 'missing elements')
    require(int(seen.sum()) == int(np.count_nonzero(flat)), 'missing pixels')
    return {'cells': len(found), 'pixels': int(seen.sum()), 'workers': len(dumps),
            'exact_input_membership': True}


def run(command, cwd, log, records, timeout, marker=None):
    start = time.monotonic()
    timed_out = False
    with log.open('wb') as output:
        process = subprocess.Popen(command, cwd=cwd, stdout=output, stderr=subprocess.STDOUT,
                                   start_new_session=True, env={**os.environ, 'OMP_NUM_THREADS': '1'})
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            code = process.wait()
            timed_out = True
    record = {'command': command, 'cwd': str(cwd), 'log': str(log), 'exit_code': code,
              'seconds': time.monotonic() - start, 'timed_out': timed_out}
    records.append(record)
    (log.parent / 'commands.json').write_text(json.dumps(records, indent=2) + '\n')
    require(not timed_out, f'timed out; see {log}')
    require(code == 0, f'command failed ({code}); see {log}')
    if marker:
        content = log.read_text(errors='replace')
        require(marker in content.splitlines(), f'missing {marker}; see {log}')
        require(not re.search(r'(?mi)^\s*(?:ERROR STOP\b|STOP(?:\s|$)|Fortran runtime error:|'
                              r'Program received signal\b|Netcdf error:|EARTHMESH_COLM_MESH_DRIVER_ERROR:)',
                              content), f'model fatal diagnostic; see {log}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--colm-repo', type=Path, required=True)
    parser.add_argument('--revision', default='HEAD')
    parser.add_argument('--out', type=Path, required=True, help='new, isolated output directory')
    parser.add_argument('--mesh', action='append', required=True, help='NAME=colm_mesh.nc (repeatable)')
    parser.add_argument('--mpi-ranks', type=int, default=0, help='0 serial; at least 3 for MPI')
    parser.add_argument('--timeout', type=float, default=900, help='seconds per build/model command')
    args = parser.parse_args()
    repo, out = args.colm_repo.resolve(), args.out.resolve()
    require(not out.exists(), 'output directory already exists')
    require(not out.is_relative_to(repo), 'output must not be inside live CoLM checkout')
    require(re.fullmatch(r'/[A-Za-z0-9_./-]+', str(out)) is not None and len(str(out)) < 140,
            'output requires a short absolute shell-safe path for CoLM internal filenames')
    require(args.mpi_ranks == 0 or args.mpi_ranks >= 3, 'MPI needs at least 3 ranks')
    require(np.isfinite(args.timeout) and args.timeout > 0, 'timeout must be finite and positive')
    meshes = {}
    for specification in args.mesh:
        name, separator, path = specification.partition('=')
        require(separator and re.fullmatch(r'[A-Za-z0-9_-]{1,24}', name), 'expected NAME=mesh.nc')
        require(name not in meshes and name not in ('model', 'logs'), 'duplicate or reserved mesh name')
        meshes[name] = Path(path).resolve(strict=True)
        require(meshes[name].is_file() and len(str(meshes[name])) <= 256, 'invalid or too long mesh path')
    revision = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', '--verify',
                                        args.revision + '^{commit}'], text=True).strip()
    source_status = subprocess.check_output(['git', '-C', str(repo), 'status', '--porcelain'], text=True)
    out.mkdir(parents=True)
    logs, build = out / 'logs', out / 'model'
    logs.mkdir()
    build.mkdir()
    archive = out / 'model.tar'
    with archive.open('xb') as stream:
        subprocess.run(['git', '-C', str(repo), 'archive', revision], stdout=stream, check=True)
    with tarfile.open(archive) as stream:
        stream.extractall(build, filter='data')
    header = build / 'include/define.h'
    original = header.read_text()
    modified = original
    for macro, enabled in [('GRIDBASED', False), ('CATCHMENT', False), ('UNSTRUCTURED', True),
                           ('SinglePoint', False), ('USEMPI', bool(args.mpi_ranks))]:
        modified, count = re.subn(r'^#(?:define|undef)\s+' + macro + r'\b',
                                 '#' + ('define' if enabled else 'undef') + ' ' + macro,
                                 modified, count=1, flags=re.M)
        require(count == 1, f'missing header macro {macro}')
    header.write_text(modified)
    (out / 'define.original.h').write_text(original)
    driver = Path(__file__).with_name('colm_mesh_driver.F90')
    manifest = {'colm_revision': revision, 'source_status': source_status, 'mpi_ranks': args.mpi_ranks,
                'archive_sha256': sha256(archive), 'driver_sha256': sha256(driver), 'runner_sha256': sha256(__file__),
                'header_sha256': sha256(header), 'inputs': {name: {'path': str(path), 'sha256': sha256(path)}
                                                         for name, path in meshes.items()},
                'scope': 'mesh and landelm only; not full mksrfdata or solver'}
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    records = []
    for tool in ('mpifort', 'nf-config', 'nc-config'):
        run([tool, '--version' if tool == 'mpifort' else '--all'], build, logs / (tool + '.log'), records, args.timeout)
    run(['make', '-j4', 'FF=mpifort', 'FOPTS=' + shlex.join(FLAGS),
         'MOD_LandElm.o', 'MOD_SrfdataRestart.o'], build, logs / 'objects.log', records, args.timeout)
    include = subprocess.check_output(['nf-config', '--includedir'], text=True).strip()
    libs = shlex.split(subprocess.check_output(['nf-config', '--flibs'], text=True))
    libs += shlex.split(subprocess.check_output(['nc-config', '--libs'], text=True))
    if sys.platform == 'darwin':
        libs += ['-Wl,-rpath,' + flag[2:] for flag in libs if flag.startswith('-L')]
        libs += ['-framework', 'Accelerate']
    binary = build / 'colm_mesh_driver'
    run(['mpifort', *FLAGS, '-Iinclude', '-I.bld', '-I' + include, str(driver),
         *map(str, sorted((build / '.bld').glob('*.o'))), *libs, '-o', str(binary)],
        build, logs / 'link.log', records, args.timeout)
    manifest['binary_sha256'] = sha256(binary)
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    results = {}
    launcher = ['mpiexec', '-n', str(args.mpi_ranks)] if args.mpi_ranks else []
    for name, path in meshes.items():
        case = out / name
        landdata = case / 'landdata'
        landdata.mkdir(parents=True)
        phases = {}
        for phase in ('build', 'load'):
            prefix = case / phase
            run([*launcher, str(binary), phase, str(path), str(landdata), str(prefix)],
                case, logs / f'{name}-{phase}.log', records, args.timeout,
                f'EARTHMESH_COLM_MESH_{phase.upper()}_OK')
            phases[phase] = audit(path, landdata, prefix)
        require(phases['build'] == phases['load'], 'build/load audit mismatch')
        require(sha256(path) == manifest['inputs'][name]['sha256'], 'input changed during run')
        results[name] = phases
        (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        print(name, json.dumps(phases), flush=True)
    print('EARTHMESH_COLM_MESH_ROUNDTRIP_OK', flush=True)


if __name__ == '__main__':
    main()

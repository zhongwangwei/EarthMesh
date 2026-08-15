#!/usr/bin/env python3
"""Offline angle-window oracle for HARP-DV point sets.

Stage one only: read a point set, rebuild its spherical Delaunay triangulation
as the convex hull of the unit vectors, and reproduce the quality numbers the
Rust side reports.  Nothing here judges a mesh until it has been shown to agree
with EarthMesh on a mesh whose answer is already known.

Why the hull.  A repository search finds no spherical Delaunay for an arbitrary
point set: ``MeshState::from_parts`` wants the caller to supply the topology and
``insert_site`` wants a mesh to insert into.  The convex hull of points on a
sphere *is* their Delaunay triangulation, so the offline side gets one for free
and no throwaway triangulator has to be written in Rust.

Measurement conventions are fixed by ``docs/angle_window_40_80_experiment_spec.md``:

* spherical angles decide, planar comparison angles are reported beside them;
* ``rho = R / e_min`` comes from chord lengths;
* an *owner* is the corner an out-of-window angle actually sits at, paired with
  which side of the window it fell off.

usage:
    angle_window_oracle.py FROZEN_FIELD_CSV [--expect-sites N] [--expect-violations N]

exit 0 when every topology check passes and every stated expectation holds.
"""

from __future__ import annotations

import argparse
import math
import sys
from collections import Counter, defaultdict

import numpy as np
from scipy.spatial import ConvexHull

WINDOW_LOW = 40.0
WINDOW_HIGH = 80.0
# docs/angle_window_40_80_experiment_spec.md section 2.4.
CELL_SCALE_TO_EDGE_LENGTH = 1.90462561372791472


def read_frozen_field(path):
    """Site ids, positions and target scales, plus the header's constants."""
    header = {}
    site_ids, points, scales = [], [], []
    with open(path) as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            if line.startswith("#"):
                for token in line.lstrip("# ").split():
                    if "=" in token:
                        key, value = token.split("=", 1)
                        try:
                            header[key] = float(value)
                        except ValueError:
                            header[key] = value
                continue
            if line.startswith("site_id"):
                continue
            site_id, x, y, z, scale = line.split(",")
            site_ids.append(int(site_id))
            points.append((float(x), float(y), float(z)))
            scales.append(float(scale))
    return np.array(site_ids), np.array(points), np.array(scales), header


def check_input(points, radius):
    """What has to hold of the raw point set, before anything normalises it.

    Normalising to the unit sphere erases a radius error, so an off-sphere point
    cannot be detected after that step -- it has to be caught here.
    """
    failures = []
    if not np.all(np.isfinite(points)):
        failures.append("some coordinates are not finite")
        return failures, None, None
    lengths = np.linalg.norm(points, axis=1)
    if np.any(lengths <= 0.0):
        failures.append("some points are at the origin")
        return failures, None, None
    drift = np.abs(lengths - radius)
    separation = nearest_pair_distance(points)
    if separation <= 0.0:
        failures.append("two points coincide")
        return failures, drift.max(), separation
    if separation / radius < 1.0e-12:
        failures.append(f"two points are {separation:.3e} m apart, effectively coincident")

    # The threshold is tied to what the check protects against rather than to a
    # chosen epsilon. Radius drift only matters when it is large enough for
    # qhull to judge a point interior, which needs it to be comparable with the
    # local point spacing; a thousandth of the nearest-pair distance leaves
    # three orders of margin. A refined mesh does drift -- candidate placement
    # normalises to the site's own current radius, not to the nominal one -- so
    # a tighter gate would only be measuring that.
    if drift.max() > 1.0e-3 * separation:
        failures.append(
            f"radius drifts up to {drift.max():.3e} m, which is "
            f"{drift.max() / separation:.3e} of the nearest-pair distance"
        )
    return failures, drift.max(), separation


def nearest_pair_distance(points):
    """Smallest distance over every pair. `O(N^2)`, which is free at this size."""
    best = math.inf
    for index in range(len(points) - 1):
        deltas = points[index + 1 :] - points[index]
        best = min(best, float(np.sqrt(np.einsum("ij,ij->i", deltas, deltas)).min()))
    return best


def check_topology(unit, hull):
    """Every precondition that makes hull facets a spherical triangulation.

    Returns the list of failures; empty means the triangulation may be measured.
    """
    failures = []
    vertices = len(unit)

    # Duplicates, and points qhull judged interior. Not an off-sphere test:
    # `check_input` owns that, because normalising has already hidden it here.
    if len(hull.vertices) != vertices:
        failures.append(
            f"hull kept {len(hull.vertices)} of {vertices} points; "
            "duplicates, or points it judged interior"
        )

    faces = hull.simplices
    edges = Counter()
    for corners in faces:
        for index in range(3):
            a, b = corners[index], corners[(index + 1) % 3]
            edges[(min(a, b), max(a, b))] += 1
    if len(faces) != 2 * vertices - 4:
        failures.append(f"F = {len(faces)}, expected 2V-4 = {2 * vertices - 4}")
    if len(edges) != 3 * vertices - 6:
        failures.append(f"E = {len(edges)}, expected 3V-6 = {3 * vertices - 6}")
    non_manifold = sum(1 for count in edges.values() if count != 2)
    if non_manifold:
        failures.append(f"{non_manifold} edges are not shared by exactly two faces")

    # The origin must be strictly inside, or these are not spherical triangles.
    if not np.all(hull.equations[:, 3] < 0.0):
        failures.append("the origin is not strictly inside the hull")

    # Qt still cannot rule out cospherical degeneracies.
    corners = unit[faces]
    areas = 0.5 * np.linalg.norm(
        np.cross(corners[:, 1] - corners[:, 0], corners[:, 2] - corners[:, 0]), axis=1
    )
    degenerate = int(np.sum(areas <= 1.0e-14))
    if degenerate:
        failures.append(f"{degenerate} facets have vanishing area")

    return failures


def oriented_faces(unit, hull):
    """Facets wound so their normals point away from the origin."""
    faces = hull.simplices.copy()
    corners = unit[faces]
    normals = np.cross(corners[:, 1] - corners[:, 0], corners[:, 2] - corners[:, 0])
    inward = np.einsum("ij,ij->i", normals, corners[:, 0]) < 0.0
    faces[inward] = faces[inward][:, [0, 2, 1]]
    return faces


def spherical_angles(points):
    """Angles between the geodesics, in the tangent plane at each apex.

    The same construction as ``criteria::triangle_angles_deg``.
    """
    out = np.empty((len(points), 3))
    for corner in range(3):
        apex = points[:, corner]
        left = points[:, (corner + 1) % 3]
        right = points[:, (corner + 2) % 3]

        def project(other):
            scale = np.einsum("ij,ij->i", apex, other) / np.einsum("ij,ij->i", apex, apex)
            return other - apex * scale[:, None]

        u, v = project(left), project(right)
        lengths = np.linalg.norm(u, axis=1) * np.linalg.norm(v, axis=1)
        cosine = np.clip(np.einsum("ij,ij->i", u, v) / np.maximum(lengths, 1e-300), -1, 1)
        out[:, corner] = np.degrees(np.arccos(cosine))
    return out


def planar_angles(points):
    """Angles of the comparison triangle built from the three chord lengths."""
    sides = np.stack(
        [
            np.linalg.norm(points[:, (c + 1) % 3] - points[:, (c + 2) % 3], axis=1)
            for c in range(3)
        ],
        axis=1,
    )
    out = np.empty((len(points), 3))
    for corner in range(3):
        opposite = sides[:, corner]
        left = sides[:, (corner + 1) % 3]
        right = sides[:, (corner + 2) % 3]
        cosine = (left**2 + right**2 - opposite**2) / np.maximum(2 * left * right, 1e-300)
        out[:, corner] = np.degrees(np.arccos(np.clip(cosine, -1, 1)))
    return out


def radius_edge(points):
    """`R / e_min` of the planar comparison triangle."""
    sides = np.stack(
        [
            np.linalg.norm(points[:, (c + 1) % 3] - points[:, (c + 2) % 3], axis=1)
            for c in range(3)
        ],
        axis=1,
    )
    semi = sides.sum(axis=1) / 2
    area = np.sqrt(
        np.maximum(
            semi * (semi - sides[:, 0]) * (semi - sides[:, 1]) * (semi - sides[:, 2]), 0.0
        )
    )
    shortest = sides.min(axis=1)
    with np.errstate(divide="ignore", invalid="ignore"):
        circumradius = sides.prod(axis=1) / (4 * area)
    return circumradius / shortest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv")
    parser.add_argument("--expect-sites", type=int)
    parser.add_argument("--expect-violations", type=int)
    parser.add_argument(
        "--expect-nxp6-baseline",
        action="store_true",
        help="lock every cross-checkable figure of the 469-point frozen field",
    )
    parser.add_argument(
        "--generate",
        action="store_true",
        help="stage two: build a point set for this field and measure it",
    )
    parser.add_argument("--lloyd-iterations", type=int, default=60)
    parser.add_argument("--lloyd-damping", type=float, default=0.6)
    parser.add_argument("--oversample", type=int, default=40000)
    args = parser.parse_args()

    site_ids, points, scales, header = read_frozen_field(args.csv)
    radius = header.get("sphere_radius_m", np.linalg.norm(points, axis=1).mean())
    print(f"read {len(points)} points, radius {radius:.6e} m")

    input_failures, drift, separation = check_input(points, radius)
    for failure in input_failures:
        print(f"  FAIL  {failure}")
    if input_failures:
        return 1
    print(
        f"  input: radius drift up to {drift:.3e} m"
        f" ({drift / separation:.2e} of the nearest pair {separation:.4e} m)"
    )

    unit = points / np.linalg.norm(points, axis=1)[:, None]
    hull = ConvexHull(unit, qhull_options="Qt")
    failures = check_topology(unit, hull)
    print(f"topology: V {len(unit)}  F {len(hull.simplices)}  E {3 * len(unit) - 6}")
    for failure in failures:
        print(f"  FAIL  {failure}")
    if failures:
        return 1
    print("  all topology checks passed")

    faces = oriented_faces(unit, hull)
    # A consistent winding shows up as every directed edge appearing once.
    directed = Counter()
    for corners in faces:
        for index in range(3):
            directed[(corners[index], corners[(index + 1) % 3])] += 1
    if any(count != 1 for count in directed.values()):
        print("  FAIL  facet winding is not consistent")
        return 1
    print("  winding is consistent")

    corners = points[faces]
    sph = spherical_angles(corners)
    pln = planar_angles(corners)
    deviation = np.abs(sph - pln)
    print(
        f"angle construction: |spherical - planar| median {np.median(deviation):.3e}"
        f"  p99 {np.percentile(deviation, 99):.3e}  max {deviation.max():.3e} deg"
    )

    below = int(np.sum(sph < WINDOW_LOW))
    above = int(np.sum(sph > WINDOW_HIGH))
    print(
        f"spherical angles: min {sph.min():.4f}  max {sph.max():.4f}"
        f"  below40 {below}  above80 {above}  outside {below + above}"
    )

    owners = set()
    for face, angles in zip(faces, sph):
        for corner, angle in zip(face, angles):
            if angle < WINDOW_LOW:
                owners.add((int(corner), False))
            elif angle > WINDOW_HIGH:
                owners.add((int(corner), True))
    print(f"violation owners (site, is_high): {len(owners)}")

    rho = radius_edge(corners)
    print(
        f"radius-edge: median {np.median(rho):.4f}  p99 {np.percentile(rho, 99):.4f}"
        f"  max {rho.max():.4f}   (equilateral 0.57735, 40deg bound 0.77786)"
    )

    degree = Counter()
    for corners_of_face in faces:
        for corner in corners_of_face:
            degree[int(corner)] += 1
    counts = Counter(degree.values())
    euler = sum((6 - d) * n for d, n in counts.items())
    print(
        "degrees: "
        + "  ".join(f"d{d}={n}" for d, n in sorted(counts.items()))
        + f"   sum(6-d) = {euler} (sphere requires 12)"
    )
    print(f"  max degree {max(counts)}   degree<5 {sum(n for d, n in counts.items() if d < 5)}")

    # Point-set diagnostics. Descriptive only -- the specification records why
    # they do not yield an angle guarantee under a variable field.
    target_edge = CELL_SCALE_TO_EDGE_LENGTH * scales
    # Over *all* pairs, not just Delaunay edges. The closest pair is always an
    # edge, but after dividing by a varying target the smallest ratio need not
    # be, so restricting to edges would report a value that is too large.
    q_sep = math.inf
    for index in range(len(points) - 1):
        deltas = points[index + 1 :] - points[index]
        distances = np.sqrt(np.einsum("ij,ij->i", deltas, deltas))
        scale = 0.5 * (target_edge[index] + target_edge[index + 1 :])
        q_sep = min(q_sep, float((distances / scale).min()))
    # The distribution belongs on the edges: over all pairs it is dominated by
    # points that are simply far apart and says nothing.
    edge_ratios = []
    for face in faces:
        for index in range(3):
            a, b = face[index], face[(index + 1) % 3]
            if a > b:
                continue
            distance = np.linalg.norm(points[a] - points[b])
            edge_ratios.append(distance / (0.5 * (target_edge[a] + target_edge[b])))
    edge_ratios = np.array(edge_ratios)
    print(
        f"q_sep {q_sep:.4f} over all pairs (regular lattice 1.0); over Delaunay edges"
        f" p1 {np.percentile(edge_ratios, 1):.4f}  median {np.median(edge_ratios):.4f}"
    )
    print(f"  min separation {separation:.4e} m")
    print("  q_sep is a worst-pair number; the edge distribution is what describes the set")

    status = 0
    if args.expect_nxp6_baseline:
        expectations = [
            ("V", len(points), 469),
            ("F", len(faces), 934),
            ("E", 3 * len(points) - 6, 1401),
            ("below40", below, 146),
            ("above80", above, 136),
            ("owners", len(owners), 184),
            ("d4", counts.get(4, 0), 5),
            ("d5", counts.get(5, 0), 45),
            ("d6", counts.get(6, 0), 376),
            ("d7", counts.get(7, 0), 43),
            ("sum(6-d)", euler, 12),
        ]
        for name, got, want in expectations:
            ok = got == want
            status |= 0 if ok else 1
            print(f"  baseline {name}: got {got}, want {want} -- {'ok' if ok else 'MISMATCH'}")
    if args.generate:
        print("\n--- stage two: variable-radius Poisson-disk + density-weighted Lloyd ---")
        print("    not Fornberg-Flyer; see the module header")
        sample = field_sampler(points, scales)
        calibrated = calibrate_and_generate(sample, radius, len(points), args)
        if calibrated is None:
            return 1
        factor, seeded = calibrated
        measure(seeded, "seeded          ")
        relaxed = lloyd_sweep(
            seeded, sample, factor, args.lloyd_iterations, args.lloyd_damping
        )
        result = measure(relaxed, f"after {args.lloyd_iterations:3} Lloyd sweeps")
        if result is not None:
            growth = result["sites"] / len(points) - 1.0
            print(f"  site growth {100.0 * growth:+.2f}% against the frozen field's {len(points)}")
            print(
                "  reading: a pass shows a feasible point set exists FOR THIS FROZEN FIELD;"
            )
            print(
                "  a failure negates this generator and this parameter range, nothing wider."
            )

    if args.expect_sites is not None:
        ok = len(points) == args.expect_sites
        print(f"expect sites {args.expect_sites}: {'ok' if ok else 'MISMATCH'}")
        status |= 0 if ok else 1
    if args.expect_violations is not None:
        ok = below + above == args.expect_violations
        print(
            f"expect window violations {args.expect_violations}: "
            f"got {below + above}, {'ok' if ok else 'MISMATCH'}"
        )
        status |= 0 if ok else 1
    return status




# ---------------------------------------------------------------------------
# Stage two: generate a point set for the frozen field, and measure it.
#
# Named for what it is: variable-radius Poisson-disk seeding followed by a
# density-weighted Lloyd relaxation. It is *not* Fornberg-Flyer -- that paper's
# advancing-front rule is not reproduced here, and calling it so would be a
# third round of the same misnaming.
#
# The relaxation is the part guide section 11.62 has *not* already refuted. That
# entry records a simplified density-weighted Lloyd candidate making the NXP6
# residue worse (72 -> 144/175), but that was a per-site candidate inside HARP's
# greedy transaction, where every single move had to improve or be rolled back.
# A sweep that moves every point at once, on a point set with no committed
# history, is a different operator; whether it helps is the open question.
#
# Interpretation, fixed in advance by the specification: success shows a
# feasible point set exists *for this frozen field*; failure negates this
# generator and this parameter range, and nothing wider.
# ---------------------------------------------------------------------------


def field_sampler(points, scales):
    """Read the frozen field at any point, by inverse-distance interpolation.

    The field is only defined where it was frozen. Interpolating is the reading
    rule the specification prescribes; calling the criterion directly would give
    the un-gradient-limited value and invalidate the comparison.
    """
    from scipy.spatial import cKDTree

    tree = cKDTree(points)
    neighbours = min(4, len(points))

    def sample(query, factor=1.0):
        query = np.atleast_2d(query)
        distances, indices = tree.query(query, k=neighbours)
        distances = np.atleast_2d(distances)
        indices = np.atleast_2d(indices)
        weights = 1.0 / np.maximum(distances, 1.0e-9)
        values = (weights * scales[indices]).sum(axis=1) / weights.sum(axis=1)
        return factor * values

    return sample


def fibonacci_sphere(count, radius):
    """A deterministic quasi-uniform cover, used for candidates and sampling."""
    index = np.arange(count, dtype=float) + 0.5
    z = 1.0 - 2.0 * index / count
    theta = np.arccos(np.clip(z, -1.0, 1.0))
    phi = np.pi * (1.0 + 5.0**0.5) * index
    return radius * np.stack(
        [np.sin(theta) * np.cos(phi), np.sin(theta) * np.sin(phi), z], axis=1
    )


def poisson_disk_sphere(sample, radius, factor, oversample):
    """Maximal variable-radius Poisson-disk sample, deterministic.

    Candidates are swept in Fibonacci order and accepted when no accepted point
    lies within the local target edge length. Sweeping a fixed sequence rather
    than throwing darts is what makes two runs agree exactly.
    """
    from scipy.spatial import cKDTree

    candidates = fibonacci_sphere(oversample, radius)
    wanted = CELL_SCALE_TO_EDGE_LENGTH * sample(candidates, factor)
    accepted = []
    accepted_radius = []
    for candidate, spacing in zip(candidates, wanted):
        if accepted:
            tree = None  # rebuilt lazily below; linear scan is fine at this size
            deltas = np.asarray(accepted) - candidate
            distances = np.sqrt(np.einsum("ij,ij->i", deltas, deltas))
            # Either point's own spacing may forbid the pair.
            if np.any(distances < np.maximum(spacing, np.asarray(accepted_radius))):
                continue
        accepted.append(candidate)
        accepted_radius.append(spacing)
    del cKDTree
    return np.asarray(accepted)


def voronoi_centroids(points, sample, factor):
    """Density-weighted Voronoi centroid of every site.

    The cell of a Delaunay site is the polygon on the circumcentres of its
    incident triangles. Each sub-triangle contributes its area times the local
    density; for a two-dimensional centroidal tessellation the spacing follows
    `rho^(-1/4)`, so `rho = ell*^(-4)` is the weight that asks for `ell*`.
    """
    unit = points / np.linalg.norm(points, axis=1)[:, None]
    hull = ConvexHull(unit, qhull_options="Qt")
    faces = oriented_faces(unit, hull)
    radius = float(np.linalg.norm(points, axis=1).mean())

    corners = points[faces]
    normals = np.cross(corners[:, 1] - corners[:, 0], corners[:, 2] - corners[:, 0])
    lengths = np.linalg.norm(normals, axis=1)
    circumcentres = radius * normals / np.maximum(lengths, 1e-300)[:, None]

    fan = defaultdict(list)
    for face_index, face in enumerate(faces):
        for corner in face:
            fan[int(corner)].append(face_index)

    moved = points.copy()
    for site, face_indices in fan.items():
        cell = circumcentres[face_indices]
        # Order the cell's corners around the site so the sub-triangles tile it.
        axis = points[site] / np.linalg.norm(points[site])
        reference = cell[0] - axis * np.dot(cell[0], axis)
        reference /= max(np.linalg.norm(reference), 1e-300)
        binormal = np.cross(axis, reference)
        offsets = cell - axis * (cell @ axis)[:, None]
        order = np.argsort(np.arctan2(offsets @ binormal, offsets @ reference))
        cell = cell[order]

        weighted = np.zeros(3)
        total = 0.0
        for index in range(len(cell)):
            a, b = cell[index], cell[(index + 1) % len(cell)]
            centroid = (points[site] + a + b) / 3.0
            area = 0.5 * np.linalg.norm(np.cross(a - points[site], b - points[site]))
            spacing = CELL_SCALE_TO_EDGE_LENGTH * float(sample(centroid, factor)[0])
            weight = area / spacing**4
            weighted += weight * centroid
            total += weight
        if total > 0.0:
            direction = weighted / total
            moved[site] = radius * direction / np.linalg.norm(direction)
    return moved


def lloyd_sweep(points, sample, factor, iterations, damping):
    """Move every site at once, damped, for a fixed number of sweeps.

    Sweep level on purpose. A per-site greedy variant of this is already on
    record as making things worse; the whole question is whether moving
    everything together behaves differently.
    """
    radius = float(np.linalg.norm(points, axis=1).mean())
    current = points.copy()
    for _ in range(iterations):
        target = voronoi_centroids(current, sample, factor)
        current = current + damping * (target - current)
        current = radius * current / np.linalg.norm(current, axis=1)[:, None]
    return current


def measure(points, label):
    """Every figure the specification asks an arm to report."""
    unit = points / np.linalg.norm(points, axis=1)[:, None]
    hull = ConvexHull(unit, qhull_options="Qt")
    failures = check_topology(unit, hull)
    if failures:
        for failure in failures:
            print(f"  FAIL  {failure}")
        return None
    faces = oriented_faces(unit, hull)
    corners = points[faces]
    sph = spherical_angles(corners)
    pln = planar_angles(corners)
    below = int(np.sum(sph < WINDOW_LOW))
    above = int(np.sum(sph > WINDOW_HIGH))
    degree = Counter()
    for face in faces:
        for corner in face:
            degree[int(corner)] += 1
    counts = Counter(degree.values())
    owners = set()
    for face, angles in zip(faces, sph):
        for corner, angle in zip(face, angles):
            if angle < WINDOW_LOW:
                owners.add((int(corner), False))
            elif angle > WINDOW_HIGH:
                owners.add((int(corner), True))
    # Neighbour cell-scale ratio, the one Go condition besides the angles that
    # a point set plus the frozen field can answer on its own.
    radius = float(np.linalg.norm(points, axis=1).mean())
    normals = np.cross(corners[:, 1] - corners[:, 0], corners[:, 2] - corners[:, 0])
    circumcentres = (
        radius * normals / np.maximum(np.linalg.norm(normals, axis=1), 1e-300)[:, None]
    )
    fan = defaultdict(list)
    for face_index, face in enumerate(faces):
        for corner in face:
            fan[int(corner)].append(face_index)
    cell_scale = np.zeros(len(points))
    for site, face_indices in fan.items():
        cell = circumcentres[face_indices]
        axis = points[site] / np.linalg.norm(points[site])
        reference = cell[0] - axis * np.dot(cell[0], axis)
        reference /= max(np.linalg.norm(reference), 1e-300)
        binormal = np.cross(axis, reference)
        offsets = cell - axis * (cell @ axis)[:, None]
        cell = cell[np.argsort(np.arctan2(offsets @ binormal, offsets @ reference))]
        area = sum(
            0.5
            * np.linalg.norm(
                np.cross(cell[i] - points[site], cell[(i + 1) % len(cell)] - points[site])
            )
            for i in range(len(cell))
        )
        cell_scale[site] = math.sqrt(area / math.pi)
    scale_violations = 0
    for face in faces:
        for index in range(3):
            a, b = int(face[index]), int(face[(index + 1) % 3])
            if a > b or cell_scale[a] <= 0 or cell_scale[b] <= 0:
                continue
            ratio = max(cell_scale[a] / cell_scale[b], cell_scale[b] / cell_scale[a])
            scale_violations += int(ratio > 1.75)

    print(
        f"{label}: sites {len(points)}  min {sph.min():.4f}  max {sph.max():.4f}"
        f"  below40 {below}  above80 {above}  outside {below + above}"
        f"  owners {len(owners)}  maxdeg {max(counts)}"
        f"  deg<5 {sum(n for d, n in counts.items() if d < 5)}"
        f"  scale>1.75 {scale_violations}"
        f"  |sph-pln| max {np.abs(sph - pln).max():.3e}"
    )
    return {
        "sites": len(points),
        "below": below,
        "above": above,
        "outside": below + above,
        "owners": len(owners),
        "max_degree": max(counts),
        "low_degree": sum(n for d, n in counts.items() if d < 5),
        "min_angle": float(sph.min()),
        "max_angle": float(sph.max()),
        "scale_violations": scale_violations,
    }


def calibrate_and_generate(sample, radius, target_sites, args):
    """Scale the field until the seeded count lands within ten per cent.

    `h(x) -> N` has no closed form, so the count is matched by bisection on a
    uniform scale factor. Without this an arm could buy quality with cells and
    the comparison would say nothing.
    """
    low, high = 0.5, 2.0
    chosen = None
    for _ in range(14):
        factor = 0.5 * (low + high)
        seeded = poisson_disk_sphere(sample, radius, factor, args.oversample)
        count = len(seeded)
        print(f"  calibrate: factor {factor:.5f} -> {count} sites")
        if abs(count - target_sites) <= 0.10 * target_sites:
            chosen = (factor, seeded)
            break
        # A larger factor means larger spacing, hence fewer sites.
        if count > target_sites:
            low = factor
        else:
            high = factor
    if chosen is None:
        print("  calibration did not reach the +/-10% band")
        return None
    factor, seeded = chosen
    print(f"  calibrated factor {factor:.5f}, {len(seeded)} sites")
    return factor, seeded


if __name__ == "__main__":
    sys.exit(main())

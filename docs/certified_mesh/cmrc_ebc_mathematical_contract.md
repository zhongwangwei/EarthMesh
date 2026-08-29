# CMRC-EBC mathematical contract

This document fixes the mathematical obligations of certified mother-grid
reverse coarsening (CMRC) and elastic block condensation (EBC).  An
implementation is conforming only when its final certifier verifies the
delivered spherical mesh; optimization diagnostics are not certificates.

## Nested mother grids

For base subdivision `n0`, level `L` uses

\[
n_L = 2^L n_0.
\]

On each planar icosahedron face, a vertex has barycentric integer address

\[
(i,j,k),\qquad i+j+k=n_L.
\]

A level `L-1` vertex embeds in level `L` as

\[
(i,j,k)\mapsto(2i,2j,2k),
\]

so \(V_{L-1}\subset V_L\).  Every level `L-1` parent triangle is covered by
exactly four level `L` children.  Consequently, a connected core that does
not touch a fine/coarse interface is condensed by the exact replacement

\[
4\text{ children}\rightarrow1\text{ parent},
\]

without geometric search.

Engineering invariants:

- parent/child relations are computed only from `TriangleAddress`;
- floating-point nearest-neighbour inference is forbidden;
- seams and the twelve icosahedron vertices have canonical addresses;
- parent/child mappings close in both directions.

## Coarse core and transition annulus

Let \(D_L\) be the parent faces physically allowed to coarsen from level `L`
to `L-1`, and let \(\partial D_L\) be its graph boundary.  For transition
width `w`, define

\[
C_w(D_L)=\{f\in D_L:d_G(f,\partial D_L)>w\}
\]

and

\[
A_w(D_L)=D_L\setminus C_w(D_L).
\]

`C_w` is the exact parent-condensation core, `A_w` is the only topology and
elastic transition region, and the complement of `D_L` remains fine.  When
\(C_w(D_L)\neq\varnothing\), the component contains a genuine coarse region
that needs no local geometric search.

## Elastic motion is not coarsening

Moving vertices while preserving the abstract complex changes coordinates,
not cardinalities:

\[
|V'|=|V|,\quad |E'|=|E|,\quad |F'|=|F|.
\]

CMRC therefore owns discrete topology and condensation.  EBC may only adjust
continuous coordinates inside the transition annulus.

## Fixed-topology elastic feasibility

For a transition topology \(T_A\), let movable vertices be
\(X=(x_1,\ldots,x_m)\), with each \(x_i\in S^2\) restricted to a closed
spherical trust region \(B_i\).  The product
\(\mathcal X=B_1\times\cdots\times B_m\) is compact.  With positive margin
\(\delta\), define the feasible set by

\[
40.2^\circ+\delta\le\theta_t(X)\le79.8^\circ-\delta,
\]

positive orientation, \(\det F_t(X)\ge\delta\), bounded distortion
\(\kappa(F_t(X))\le K_\star-\delta\), and passing physical, degree, and dual
constraints.  If this feasible set is non-empty, it is compact; hence every
continuous elastic energy attains a minimum on it.  This proves existence of
an optimum, not that a particular numerical optimizer finds one.

## Angle window and distortion guide

Relative to an equilateral reference triangle, a planar deformation with
condition number \(K=\sigma_{\max}/\sigma_{\min}\) obeys the half-angle bounds

\[
2\arctan\!\left(\frac{\tan30^\circ}{K}\right)
\le\beta\le
2\arctan\!\left(K\tan30^\circ\right).
\]

The internal `40.2°–79.8°` window gives the conservative guide

\[
K_\star=\frac{\tan39.9^\circ}{\tan30^\circ}\approx1.448219.
\]

This bound guides candidate construction only.  Delivery still requires the
actual spherical angle certificate to pass the hard `40°–80°` window.

## Global safety

Let \(\mathcal C(M)\) denote all hard geometry, topology, primal/dual,
physical, balance, and remap predicates.  The initial highest safe mother grid
satisfies \(\mathcal C(M_0)\).  A component transaction either commits a trial
that passes every certificate, or restores the exact snapshot / locally
promotes it.  Therefore, by induction,

\[
\forall k,\quad \mathcal C(M_k)=\mathrm{true}.
\]

No failed transaction may weaken an already certified mesh.

## Finite termination

Define over-refinement

\[
\Phi(M)=\sum_f\max(0,L_{\mathrm{delivered}}(f)-L_{\mathrm{required}}(f)).
\]

A successful condensation strictly decreases \(\Phi\).  An unsuccessful
component has only finitely many topology candidates, halo widths, elastic
iterations, interval boxes, promotions, and budget states.  Once every
uncommitted component is promoted or closed, the epoch terminates.  With
finitely many levels, the full process terminates.

## Conditional mixed-level guarantee

For a level `L` component, if

1. \(C_w(D_L)\neq\varnothing\),
2. the transition candidate set contains a topology \(T_A\),
3. its strict feasible set is non-empty, and
4. condensation respects the required level field,

then a certified transaction exists that decreases \(\Phi\) and leaves at
least levels `L` and `L-1` present.  This is conditional: arbitrary fragmented
threshold fields need not admit a useful mixed mesh.  The scheduler must,
however, give every component a bounded attempt so one hard component cannot
prevent another feasible component from being tried.

## Product consequence

A safe certified mesh is not automatically a fulfilled adaptive mesh.  When
mixed levels were requested but the delivered mesh is uniform, the product
outcome is `CompressionIncomplete` unless the caller explicitly selected a
safe fallback.  A safe fallback is separately named and must never be
published as ordinary adaptive success.

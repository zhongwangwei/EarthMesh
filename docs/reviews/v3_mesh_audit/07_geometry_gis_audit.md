# 07 — Geometry / GIS / Mask Overlay Audit (EarthMesh v3)

> Phase P-geometry（提案，可提 patch，不落地）· 未修改任何 `src/rust` 代码
> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md) · 关联：[05_coupled_mesh_audit.md](./05_coupled_mesh_audit.md)（overlay 守恒）· [06_merit_hydro_hydro_coast_audit.md](./06_merit_hydro_hydro_coast_audit.md)（degree buffer/simplify）
> 审查对象：EarthMesh v3，分支 `v3.0.0-alpha1`，仅当前项目，不引用任何旧版本。
> 证据：`earthmesh_geometry/src/lib.rs`（全文 610 行已读）+ `cli/lib.rs`（area-judge/hydro 几何）+ [01](./01_build_and_crate_audit.md)/[04](./04_physical_refinement_audit.md)（mesh 3D 球面）。
> 日期：2026-06-22 · 证据等级见 AUDIT_PRINCIPLES §5。

---

## 0. 核心结论（先读）

**EarthMesh v3 存在"球面网格 + 平面 GIS"的根本割裂**（A 级）：

- **网格生成核心是 3D 球面**：OLAM Delaunay/Voronoi 用 3D 笛卡尔点 + `EARTH_RADIUS_METERS`（见 [01](./01_build_and_crate_audit.md)#1 olam 半径、[04](./04_physical_refinement_audit.md)）。
- **决定细化与耦合的 GIS / mask / overlay / area 层几乎全部是 lon/lat 平面**：`earthmesh_geometry` 的 `polygon_area`/`clip_convex_polygon`/`intersection_area`/`overlay_cell` 全在 `Point{x=lon,y=lat}` 上做欧氏几何（`geometry/lib.rs:74,86,114,155`），无 cos(lat)、无投影、无球面面积。
- **唯一球面计算**：`haversine_km`（`:32`）→ circle 域包含（`is_point_in_circle_km :45`）。其它（bbox/polygon 域、close mask、hydro buffer/simplify）全平面。

14 个重点问题的直接回答见 §1–§4 表格；总览：

| 问题 | 答案 |
|------|------|
| 1 哪些在 lon/lat 平面 | polygon_area / clip / intersection / overlay / mask fraction / bbox+polygon 域 / close mask / hydro buffer&simplify |
| 2 哪些在球面 | 仅 `haversine_km`→circle 域；mesh 生成核心（3D 笛卡尔球面） |
| 3 polygon area 适合全球/高纬/跨经？ | ❌ 平面 degree² shoelace，高纬/大单元/跨经失真，绝对面积错 |
| 4 buffer_deg 高纬失真？ | ❌ 严重（见 [06](./06_merit_hydro_hydro_coast_audit.md) H-G2） |
| 5 convex clip 够处理复杂岸线/河网？ | ❌ 不够（仅凸 clip；凹靠三角化，失败静默回退） |
| 6 concave/multipolygon/holes/自交 支持？ | 🟡 simple concave 支持；multipolygon/holes ❌；自交→回退错误 |
| 7 close mask 方向错误？ | 🟡 clip 自适应朝向，但生产无统一 winding 规范 |
| 8 dateline 正确？ | 🟡 仅 area-judge ±180 粗移；overlay/hydro/bbox ❌ |
| 9 pole 正确？ | ❌ 无任何极区处理 |
| 10 GUI preview 影响生产？ | ✅ 仅显示，不回灌（见 §6） |
| 11 需要 robust geometry backend？ | ✅ 需要（§5） |
| 12 需要 local equal-area projection？ | ✅ 需要（buffer/area/fraction） |
| 13 需要 spherical area / geodesic edge？ | ✅ 需要（绝对面积/守恒） |
| 14 需要 geometry quality flags？ | ✅ 需要，且现有仅 2 个（§7） |

---

## 1. Current Geometry Inventory

| 基元 | 位置 | 坐标系 | 能力 | 生产用途 |
|------|------|--------|------|----------|
| `Point{x,y}` | `geometry:4` | lon/lat 平面 | 2D 点 | 所有 GIS |
| `polygon_area` | `geometry:74` | **平面 degree²** | shoelace，绝对值 | overlay cell/overlap 面积 |
| `signed_area` | `geometry:234` | 平面 | 带号面积（定 winding） | clip/三角化朝向 |
| `clip_convex_polygon` | `geometry:86` | 平面 | Sutherland-Hodgman，**clip 须凸** | intersection |
| `intersection_area` | `geometry:114` | 平面 | 双 ear-clip 三角化后裁剪 | overlay overlap |
| `triangulate_simple_polygon` | `geometry:246` | 平面 | ear-clipping，**simple only**，失败→None | intersection |
| `overlay_cell` / `overlay_cells` | `geometry:155,212` | 平面 | class fraction + winning + flags | **mask→cell 分数**（但 coupling 未调用，见 [05](./05_coupled_mesh_audit.md)） |
| `haversine_km` | `geometry:32` | **球面** | 大圆距离（km） | circle 域 |
| `is_point_in_circle_km` | `geometry:45` | 球面 | 点在圆内 | circle 域 |
| `is_point_in_convex_polygon` | `geometry:53` | 平面 | **仅凸**点在多边形 | bbox/polygon 域 |
| `ray_segment_intersection_lon` | `geometry:496` | 平面 | 射线-段交（lon） | area-judge 包含 |
| `segments_intersect_strict` | `geometry:532` | 平面 | 段相交（严格） | 自交检测 |
| `area_judge_first_self_intersection` | `geometry:556` | 平面 | close 曲线自交 | area-judge close |
| `shift_longitudes_for_dateline_crossing` | `geometry:598` | 平面 | **±180 粗移** | 仅 area-judge |
| hydro `simplify_closed_ring`/`buffer_*` | `cli:2734,2911` | **平面 degree** | DP 简化 / degree offset | hydro mask（见 [06](./06_merit_hydro_hydro_coast_audit.md)） |
| mesh OLAM Delaunay/Voronoi | `mesh.rs`（[04](./04_physical_refinement_audit.md)） | **3D 球面** | 球面网格生成 | mesh 生成 |

> 重要：存在**两套平面几何实现**——`earthmesh_geometry`（干净、带 flags、但 coupling 未用）与 `cli` 内联 area-judge/hydro 几何（实际生产路径）。重复且不一致。

---

## 2. Planar vs Spherical Risk Table

| 计算 | 当前坐标系 | 应当 | 风险 | 严重度 |
|------|-----------|------|------|--------|
| polygon area（cell/overlap 绝对面积） | 平面 degree² `geometry:74` | 球面面积 (m²) 或局地等面积投影 | 绝对面积错（degree² 无物理意义）；coupling `cell_area_m2` 不可信（见 [05](./05_coupled_mesh_audit.md)） | High |
| class fraction = overlap/cell | 平面比值 `geometry:178` | 局地等面积下比值 | 小单元近似 OK（cos(lat) 比值近似抵消）；**大单元/高纬/跨经显著偏** | Med-High |
| buffer (hydro) | 平面 degree `cli:2911` | km + 局地投影 | 高纬严重失真（见 [06](./06_merit_hydro_hydro_coast_audit.md) H-G2） | High |
| simplify tolerance | 平面 degree `cli:2734` | km | 窄河道破坏、高纬更甚（[06](./06_merit_hydro_hydro_coast_audit.md) H-G3） | Med |
| distance to river/coast | 平面 degree（[06](./06_merit_hydro_hydro_coast_audit.md)） | km (haversine/geodesic) | 各向异性、随纬变 | High |
| circle 域包含 | 球面 haversine ✅ `geometry:45` | — | 正确 | — |
| bbox/polygon 域包含 | 平面凸 `geometry:53` | 球面/投影 + 凹支持 | 凹域错、跨经错 | Med |
| edge length（min-edge/CFL） | mesh 3D 球面 ✅ | — | 正确（弦长，近似弧长） | Low |
| tile-bbox 相交 | 平面经度比较 `cli:4401` | 含 antimeridian | 跨 180° 选错（[06](./06_merit_hydro_hydro_coast_audit.md) H-B1） | High |

> 关键澄清（避免误报）：`overlay_cell` 的 **fraction 是比值**，对**小单元**因 cos(lat) 在分子分母近似抵消而**近似可用**；但 (a) 绝对 `cell_area_m2` 必错，(b) 大单元/高纬/跨经/极区偏差显著，(c) shoelace 在 (lon,lat) 把经向当等距，纬向越高越偏。守恒耦合（[05](./05_coupled_mesh_audit.md)）必须用真实面积。

---

## 3. Dateline / Pole Risk

| 风险 | 现状 | 位置 | 影响 |
|------|------|------|------|
| dateline（±180°）overlay/mask | **未处理** | `geometry` overlay 无 dateline 逻辑 | 跨 180° 的 cell/mask 经度跳变 → 面积/相交错乱 |
| dateline tile 选择 | **未处理** | `cli:4401`（[06](./06_merit_hydro_hydro_coast_audit.md) H-B1） | 太平洋/白令海漏选 |
| dateline area-judge close | 🟡 ±180 粗移 | `geometry:598` | 仅对接近 ±180 的简单情形有效；远离 ±180 的多边形会被错误平移 |
| pole（±90°）面积 | **未处理** | `polygon_area :74` | 极区 shoelace 退化、经线汇聚被忽略 |
| pole 域/网格 | **未处理** | 无极区特判 | 极区单元几何失真、可能零/负面积 |
| 经度归一化 | 无统一规范 | — | [-180,180] vs [0,360] 混用风险 |

**结论**：dateline 仅在 area-judge 有"创可贴"式处理，overlay/hydro/tile/bbox 全裸；pole 完全无处理。全球网格在这两处都有正确性隐患。

---

## 4. Mask Overlay Risk

| ID | 风险 | 位置 | 说明 |
|----|------|------|------|
| O1 | 三角化失败静默回退凸裁剪 | `intersection_area :115-120` | 自交/复杂多边形 triangulate→None → `clip_convex_polygon(a,b)`，结果错且**无 flag** |
| O2 | clip 多边形须凸 | `clip_convex_polygon :86` | 凹海岸线/河网 mask 作 clip 时结果错 |
| O3 | 无 Σ fraction=1 校验 | `overlay_cell :198-200` | 各 fraction 各自 clamp≤1，但**和不校验**；缺 `unresolved_fraction_sum_error` flag |
| O4 | priority 平局后者胜 | `overlay_cell :181` `>=` | mask 顺序影响 winning_class（不确定性） |
| O5 | fraction 可被重复累加 | `add_class_fraction :223` | 同 class 多 mask 累加，可 >1 后才 clamp，丢失重叠信息 |
| O6 | quality_flags 仅 2 种 | `overlay_cell :164,194` | 只有 `zero_area_cell`/`missing_mask`，缺 9 种（§7） |
| O7 | holes / multipolygon 不支持 | 全 crate | 带洞/多部件 mask 无法表达（岛中湖、群岛） |
| O8 | 非连续重复点未清理 | `normalized_polygon_vertices :292` | 仅去相邻 + 首尾重复，非相邻重复/退化边残留 |
| O9 | overlay 未接生产 coupling | [05](./05_coupled_mesh_audit.md) W2 | 引擎用点采样而非 overlay，守恒分数完全没用上 |

---

## 5. Recommended Geometry Backend

目标：一个**球面感知 + 投影感知 + robust** 的几何后端，统一替代 `earthmesh_geometry` 平面原语与 cli 内联几何。

| 维度 | 建议 |
|------|------|
| 面积 | **球面多边形面积**（L'Huilier/球面超额）或**局地等面积投影**（Lambert Azimuthal Equal-Area，中心取单元质心）后平面 shoelace |
| 距离/buffer | **geodesic/haversine（km）**；buffer 在局地方位等距投影下 offset 再投回 |
| 相交/裁剪 | robust polygon clipping（支持凹、holes、multipolygon），如成熟算法 Weiler–Atherton / Vatti，或引入经过验证的几何库（评估 `geo`/`geo-clipper`，注意许可与依赖体量） |
| dateline | 统一经度归一化 + 跨 180° 分裂（split-at-antimeridian）策略，所有路径共用 |
| pole | 极区局地投影（极方位投影）特判 |
| robust 谓词 | 用带容差或精确谓词（orientation/incircle）避免退化误判 |
| 数据结构 | 引入 `Geometry`/`MultiPolygon`/`Ring{exterior, holes}` 类型，winding 规范（exterior CCW, holes CW） |
| 接口 | 保持 `overlay_cell` 签名兼容，内部换实现（[03 §9 零迁移](./03_config_schema_audit.md#9-migration-free-compatibility-strategyv3-内部无破坏)） |

> 取舍：优先**局地等面积投影 + geodesic 距离**（实现成本低、对全球/高纬/守恒收益最大），robust clipping 库为次（评估依赖后再定）。两套平面实现应合并为单一后端，消除重复。

---

## 6. Production Geometry vs Preview Geometry Separation

| 路径 | 几何 | 是否影响生产 | 证据 |
|------|------|--------------|------|
| GUI `draw_mesh_2d` | 等矩形投影显示 | **否，仅显示** | `gui/main.rs:500`（[02](./02_workflow_consistency_audit.md)） |
| GUI walkers 地图 + `MeshOverlay` | Web Mercator 瓦片 + 叠加 | **否，仅显示** | `gui/main.rs:698-996` |
| GUI `read_gridfile_mesh_points` | 读网格点供绘制 | **否，只读预览** | `gui/main.rs`（[02](./02_workflow_consistency_audit.md)） |
| 生产 mask/overlay/area/refine | `geometry` + cli 几何 | **是** | 本报告 §1 |

**结论**：preview 与生产几何**已分离**（preview 只读不回灌）——这是好的。建议**显式制度化**：
- preview 几何（Mercator/等矩形）**永不**用于面积/fraction/守恒计算。
- 生产几何结果（含 GeometryQualityFlags）回传 GUI **展示**（worst cells、flag 高亮），但 GUI 不得反向修改生产几何。
- 文档/类型层面标注 `PreviewGeometry` vs `ProductionGeometry`，防止未来误用。

---

## 7. GeometryQualityFlags Design

现状仅 `zero_area_cell`、`missing_mask`（`geometry:164,194`）。提议完整枚举（对接 [05 `CoupledQualityFlag`](./05_coupled_mesh_audit.md#3-coupled-cell-classification-design) 与 [03 `QualityConstraintConfig`](./03_config_schema_audit.md#3-rust-type-sketches)）：

```rust
pub enum GeometryQualityFlag {
    ZeroAreaCell,                 // 现有：单元面积≤0
    InvalidPolygon,               // 顶点<3 / 非有限坐标 / 退化
    SelfIntersection,             // 多边形自交（接 area_judge_first_self_intersection）
    DuplicateVertex,              // 重复/近重复顶点（含非相邻）
    DatelineCrossing,             // 跨 ±180°，需分裂处理
    PolarRegionWarning,           // 接近 ±90°，平面几何不可靠
    PlanarAreaUsedWarning,        // 用了平面 degree² 面积（而非球面/投影）
    ProjectionDistortionWarning,  // 单元跨度大/高纬，投影畸变超阈
    MaskOverlapConflict,          // 多 mask 在同区域 priority 冲突 / fraction 重叠
    MissingMask,                  // 现有：单元无任何 mask 命中
    UnresolvedFractionSumError,   // Σ fraction 偏离 1 超容差（守恒失败）
}
pub struct GeometryQualityReport {
    pub flags: Vec<GeometryQualityFlag>,
    pub fraction_sum: f64,        // 实测 Σ fraction
    pub max_latitude_deg: f64,    // 用于 polar/distortion 判定
    pub planar_area_deg2: f64,
    pub spherical_area_m2: Option<f64>,
}
```

每个 flag 的触发点与门禁建议：

| Flag | 触发 | 门禁（`ViolationPolicy`） |
|------|------|---------------------------|
| ZeroAreaCell | area≤0 | Block |
| InvalidPolygon | <3 顶点/非有限 | Block |
| SelfIntersection | 段相交检测命中 | Block（mask 生成）/Warn |
| DuplicateVertex | 容差去重发现 | Warn（自动清理） |
| DatelineCrossing | 经度跨 ±180 | Warn（触发分裂） |
| PolarRegionWarning | \|lat\|>阈值（如 80°） | Warn |
| PlanarAreaUsedWarning | 用平面面积做守恒 | Warn（提示换球面） |
| ProjectionDistortionWarning | 单元经向跨度·cos(lat) 畸变>阈 | Warn |
| MaskOverlapConflict | 同区 priority 并列/fraction 重叠>阈 | Warn |
| MissingMask | 无 mask 命中 | Warn |
| UnresolvedFractionSumError | \|1-Σfraction\|>1e-6 | Block（耦合，见 [05](./05_coupled_mesh_audit.md)） |

---

## 8. Required Tests

| 测试 | 目的 | 关联 |
|------|------|------|
| `polygon_area_spherical_matches_reference` | 球面面积 vs 解析值（球冠/球面三角） | §2 |
| `fraction_ratio_stable_across_latitude` | 同形状单元在 0°/45°/80° fraction 一致 | §2 |
| `overlay_fraction_sum_flags_when_not_one` | Σ≠1 触发 `UnresolvedFractionSumError` | O3/§7 |
| `intersection_concave_polygon_correct` | 凹多边形相交正确（非回退） | O1 |
| `intersection_self_intersecting_flagged_not_silent` | 自交输入报 flag 而非静默回退 | O1 |
| `clip_against_concave_mask_correct` | 凹 mask 裁剪正确 | O2 |
| `dateline_polygon_split_area_correct` | 跨 180° 多边形面积正确 | §3 |
| `polar_cell_flagged` | 极区单元报 `PolarRegionWarning` | §3 |
| `multipolygon_with_hole_supported` | 带洞/多部件 mask | O7 |
| `duplicate_nonadjacent_vertex_cleaned` | 非相邻重复点清理 | O8 |
| `circle_domain_uses_haversine` | circle 域球面距离（回归保护） | §1 |
| `preview_geometry_not_used_in_production` | 架构测试：Mercator/等矩形不进生产 | §6 |

> 现状：geometry crate 仅 2 个单测（`polygon_area`/`intersection_area`，`geometry:354-384`），见 [01](./01_build_and_crate_audit.md)。上述球面/守恒/dateline/极区测试**全缺**。

---

## 9. Patch Plan（提案，待 P8 批准）

| Patch ID | 关联 | 目标 | 改动摘要 | 验证 | 风险 |
|----------|------|------|----------|------|------|
| PATCH-G1 | §7 | 扩展 `GeometryQualityFlag`（11 种）+ report | 枚举 + overlay 填充 | flag 单测 | 低（新增） |
| PATCH-G2 | O3/[05](./05_coupled_mesh_audit.md) | Σ fraction 校验 + `UnresolvedFractionSumError` | overlay 加守恒检查 | `overlay_fraction_sum_flags` | 低 |
| PATCH-G3 | §2 | 球面/局地等面积面积函数 | 新 `spherical_polygon_area_m2` + 投影 | `polygon_area_spherical_matches` | 中 |
| PATCH-G4 | O1/O2/O7 | robust clipping（凹/holes/multipolygon） | 评估引入几何库 or 自研 Weiler-Atherton | 凹/洞测试 | 高 |
| PATCH-G5 | §3 | dateline 分裂 + 经度归一化（统一） | split-at-antimeridian，全路径共用 | `dateline_polygon_split` | 中 |
| PATCH-G6 | §3 | 极区特判 + `PolarRegionWarning` | 极方位投影 | `polar_cell_flagged` | 中 |
| PATCH-G7 | §1 | 合并两套平面几何为单一后端 | cli 内联几何改调 geometry crate | 回归比对 | 中 |
| PATCH-G8 | §6 | 制度化 preview/production 分离 | 类型标注 + 架构测试 | `preview_not_in_production` | 低 |
| PATCH-G9 | [06](./06_merit_hydro_hydro_coast_audit.md) | hydro buffer/simplify 改 km+投影 | 见 [06](./06_merit_hydro_hydro_coast_audit.md) H2 | `buffer_km_consistent` | 高 |

> 顺序：G1/G2（flags+守恒，立即收益、低风险）→ G3（球面面积）→ G5/G6（dateline/pole）→ G4（robust clipping，最重）→ G7（合并）→ G8/G9。先决：[03](./03_config_schema_audit.md) S1（project crate）以承载 `QualityConstraintConfig`。

---

## 关键证据索引（file:line）

- geometry crate（全平面，除 haversine）：`polygon_area:74`、`clip_convex_polygon:86`、`intersection_area:114`、`triangulate_simple_polygon:246`、`overlay_cell:155`（flags `:164,194`，priority `:181`，Σ 无校验 `:198`）、`haversine_km:32`、`is_point_in_circle_km:45`、`is_point_in_convex_polygon:53`、`shift_longitudes_for_dateline_crossing:598`、`area_judge_first_self_intersection:556`
- hydro 平面几何：`cli/lib.rs:2734`(simplify)、`:2911`(buffer)、`:4401`(tile bbox)（见 [06](./06_merit_hydro_hydro_coast_audit.md)）
- mesh 球面：3D Delaunay/Voronoi + `EARTH_RADIUS_METERS`（[01](./01_build_and_crate_audit.md)#1、[04](./04_physical_refinement_audit.md)）
- GUI preview（显示，不回灌）：`gui/main.rs:500,698-996`（[02](./02_workflow_consistency_audit.md)）
- 设计落点：[05 `CoupledQualityFlag`](./05_coupled_mesh_audit.md#3-coupled-cell-classification-design)、[03 `QualityConstraintConfig`](./03_config_schema_audit.md#3-rust-type-sketches)

*本报告基于 `earthmesh_geometry/src/lib.rs` 全文与相关 cli/mesh 证据；为几何后端设计提案。未修改任何 `src/rust` 代码。*

# 04 — Physical Refinement Audit: Land / Ocean / Atmosphere (EarthMesh v3)

> Phase P3 物理一致性审查（提案，可提 patch，不落地）· 未修改任何 `src/rust` 代码
> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md) · 上游：[02_workflow_consistency_audit.md](./02_workflow_consistency_audit.md) · [03_config_schema_audit.md](./03_config_schema_audit.md)
> 审查对象：EarthMesh v3，分支 `v3.0.0-alpha1`，仅当前项目，不引用任何旧版本。
> 证据：`earthmesh_core/src/lib.rs:1236-1276`(RefineConfig)、`earthmesh_gui/src/main.rs:3556-3707`(criteria UI 与数组索引)、[02](./02_workflow_consistency_audit.md)（hydro/coast/coupling 现状）。
> 日期：2026-06-22 · 证据等级见 AUDIT_PRINCIPLES §5。本文档的 score/criterion 设计与 [03 的 plugin `RefinementCriterion`](./03_config_schema_audit.md#3-rust-type-sketches) 对接。

---

## 0. 核心结论（先读）

当前 v3 的细化判据本质是**「每个物理变量的 mean/std 是否超阈值」开关**（`th_*` 阈值 + `refine_*` 布尔，`core/lib.rs:1256-1275`），**没有归一化、没有权重、没有 score 合成、没有 preset、没有与物理过程的显式绑定、没有质量收益评估**。这直接违背总原则 1/2/3/5：

- **原则 2（异质性须影响关键物理过程）**：当前阈值与物理过程无显式关联（只是"std 大就细化"）。
- **原则 3（收益 > 成本）**：无 score、无预算，无法权衡。
- **原则 1/5（异质性证据 + GUI 可解释）**：无"为什么这里被细化"的可追溯分数。

**当前真实支持的判据**（A 级实证）：

| 域 | 支持的变量（mean/std 阈值） | 字段 | 覆盖用户清单 |
|----|------------------------------|------|--------------|
| Land | num_landtypes(计数), area_mainland, LAI(m/s), slope(m/s), 土壤 ks/ksol/tkdry/tksatf/tksatu(m/s) | `refine_onelayer_lnd[4]`,`refine_twolayer_lnd[10]`,scalars | **~6 / 24** |
| Ocean | sea_ratio, SST(m/s), SSH(m/s), EKE(m/s), seaslope=bathy 梯度(m/s) | `refine_onelayer_ocn[8]`,`th_sea_ratio[2]` | **~5 / 21** |
| Atmosphere | typhoon(m/s) | `refine_onelayer_atmos[2]` | **~1 / 17** |

> 本报告提出 `score_land / score_ocean / score_atmos` 加权归一化框架（§3），用 [03 的 plugin criteria](./03_config_schema_audit.md#3-rust-type-sketches) 实现，每个 term = 一个 `RefinementCriterion`，自带 `physical_process`、`gui_spec`、`quality_contribution`。

---

## 1. Current Capability Table

图例：✅ 已支持 · 🟡 部分/代理 · ❌ 缺失（🟡/❌ 见 §2）。"现状字段"为 `RefineConfig` 字段或 [02](./02_workflow_consistency_audit.md) 中的所在 workflow。

### 1.A Land（用户清单 24 项）

| # | 物理特征 | 现状 | 现状字段 / 证据 |
|---|----------|------|-----------------|
| 1 | land cover / type heterogeneity | 🟡 | `th_num_landtypes`(计数非熵) `core/lib.rs:1256` |
| 2 | dominant class purity | 🟡 | `th_area_mainland`(主陆占比) `:1257` |
| 3 | LAI mean/std/seasonality | 🟡 | LAI m/s ✅ `refine_onelayer_lnd[0,1]`；seasonality ❌ |
| 4 | terrain elevation (range) | ❌ | 无 elevation 判据（仅 slope） |
| 5 | slope | ✅ | slope m/s `refine_onelayer_lnd[2,3]` |
| 6 | terrain curvature | ❌ | — |
| 7 | topographic wetness index (TWI) | ❌ | — |
| 8 | river network density | 🟡 | 仅 hydro-close mask([02](./02_workflow_consistency_audit.md)§5)，非 score |
| 9 | drainage area | 🟡 | MERIT `upa` 用于分类，非 land score |
| 10 | distance to river | ❌ | — |
| 11 | soil texture heterogeneity | 🟡 | 土壤导水/热导 ks/ksol m/s `refine_twolayer_lnd[0-3]`（非"texture"本身） |
| 12 | hydraulic conductivity | ✅ | ks m/s `refine_twolayer_lnd[0,1]` |
| 13 | soil thermal properties | ✅ | tkdry/tksatf/tksatu m/s `refine_twolayer_lnd[4-9]` |
| 14 | groundwater depth | ❌ | — |
| 15 | wetland fraction | ❌ | — |
| 16 | cropland / irrigation fraction | ❌ | — |
| 17 | urban / impervious surface | ❌ | — |
| 18 | snow cover frequency | ❌ | — |
| 19 | glacier fraction | ❌ | — |
| 20 | permafrost probability | ❌ | — |
| 21 | precipitation gradient | ❌ | — |
| 22 | aridity index | ❌ | — |
| 23 | evapotranspiration sensitivity | ❌ | — |
| 24 | land-atmosphere coupling hotspots | ❌ | — |

### 1.B Ocean（用户清单 21 项）

| # | 物理特征 | 现状 | 现状字段 / 证据 |
|---|----------|------|-----------------|
| 1 | sea fraction | ✅ | `th_sea_ratio[2]` `core/lib.rs:1258` |
| 2 | coastline complexity | ❌ | coast 仅 MERIT 分类([02](./02_workflow_consistency_audit.md)§5)，非 score |
| 3 | distance to coast | ❌ | — |
| 4 | bathymetry depth | 🟡 | 仅 seaslope(梯度)，无深度本身 |
| 5 | bathymetry slope | ✅ | seaslope m/s `refine_onelayer_ocn[6,7]` |
| 6 | shelf break | ❌ | — |
| 7 | estuary mask | 🟡 | CaMa `is_estuary` 存在但未接入([02](./02_workflow_consistency_audit.md)§3) |
| 8 | river mouth | 🟡 | 同上，未接入 score |
| 9 | tidal channel | ❌ | — |
| 10 | tidal range | ❌ | — |
| 11 | river discharge | 🟡 | CaMa reach 有，但未接入 score |
| 12 | SST gradient | ✅ | sst m/s `refine_onelayer_ocn[0,1]` |
| 13 | SSH gradient | ✅ | ssh m/s `refine_onelayer_ocn[2,3]` |
| 14 | EKE | ✅ | eke m/s `refine_onelayer_ocn[4,5]` |
| 15 | western boundary currents | ❌ | (可由 EKE/SSH 间接) |
| 16 | sea ice edge | ❌ | — |
| 17 | Rossby radius | ❌ | — |
| 18 | storm surge risk | ❌ | — |
| 19 | island complexity | ❌ | — |
| 20 | narrow straits | ❌ | — |
| 21 | wetland/delta/coastal floodplain | ❌ | — |

### 1.C Atmosphere（用户清单 17 项）

| # | 物理特征 | 现状 | 现状字段 / 证据 |
|---|----------|------|-----------------|
| 1 | topography elevation | ❌ | 无 atmos 地形判据 |
| 2 | topographic gradient | ❌ | — |
| 3 | orographic precipitation | ❌ | — |
| 4 | extreme precipitation | ❌ | — |
| 5 | convective frequency | ❌ | — |
| 6 | storm tracks | ❌ | — |
| 7 | typhoon / TC density | ✅ | typhoon m/s `refine_onelayer_atmos[0,1]` |
| 8 | monsoon rainband | ❌ | — |
| 9 | jet stream variability | ❌ | — |
| 10 | land-sea thermal contrast | ❌ | — |
| 11 | urban heat island | ❌ | — |
| 12 | aerosols/emissions hotspots | ❌ | — |
| 13 | population/exposure (optional) | ❌ | — |
| 14 | mountain wave regions | ❌ | — |
| 15 | snow/albedo gradient | ❌ | — |
| 16 | SST front | 🟡 | ocean 有 SST，未供 atmos |
| 17 | atmosphere-land coupling hotspots | ❌ | — |

> 汇总覆盖率：**Land 6/24、Ocean 5/21、Atmosphere 1/17**（✅ 计为支持，🟡 不计）。Atmosphere 几乎空白——仅台风。

---

## 2. Missing Physical Features（缺口清单 + 优先级）

| 域 | 缺失（高价值，数据通常可得） | 缺失（中等） | 缺失（高级/数据稀缺） |
|----|------------------------------|--------------|------------------------|
| Land | elevation range、TWI、river distance/density(MERIT 已有数据)、urban/impervious、snow cover、permafrost | curvature、drainage area→score、groundwater、wetland、cropland/irrigation、precip gradient/aridity | ET sensitivity、land-atmos coupling hotspots、glacier、LAI seasonality |
| Ocean | coastline complexity、distance-to-coast、bathymetry depth、estuary/river-mouth(CaMa 已有)、shelf break、island/narrow-strait preservation | tidal range/channel、river discharge→score、sea-ice edge、storm surge | Rossby radius、WBC 显式、delta/floodplain |
| Atmosphere | topographic gradient、orographic precip、extreme precip、storm track、land-sea contrast | convective freq、monsoon rainband、urban heat、SST front(复用 ocean) | jet stream、aerosols、mountain wave、population、snow/albedo |

**关键观察**：MERIT-Hydro（river/upa/elv/wth）与 CaMa（is_estuary）数据**已在项目里被读取**（[02](./02_workflow_consistency_audit.md)§5/§3），但**只用于 close-mask，未进入 land/ocean score**——这是"低垂果实"：把已有数据接入 score 即可补 river distance / drainage / estuary / elevation。

---

## 3. Score Formulas

### 3.0 通用归一化与合成规则

对任意原始特征 \(f\)，在域内按稳健分位归一化到 \([0,1]\)：

```
norm(f) = clamp( (agg(f) - p_lo) / (p_hi - p_lo), 0, 1 )
```
- `agg ∈ {mean, std, gradient, range, quantile(q)}`（对应 [03 `Aggregation`](./03_config_schema_audit.md#3-rust-type-sketches)）。
- `p_lo, p_hi` = 域内分位（默认 5%/95%，抗异常值），或 `CriterionThreshold` 显式给定。
- 距离类用指数衰减 `exp(-d / L)`（`L` = 特征长度，单位与数据一致）。

合成与细化映射：
```
score_domain = clamp( Σ_i w_i · term_i , 0, 1 )          // w_i ≥ 0，Σ 不必=1
refine_demand_level = ceil( score_domain · max_passes )   // 0..max_passes
```
受 [03 `RefinementBudget`](./03_config_schema_audit.md#3-rust-type-sketches) 约束（`max_cells`/`min_edge_km`/`max_refine_ratio`）→ 满足原则 3。
每个 `term_i` = 一个 plugin `RefinementCriterion`，其 `score()` 返回 `{demand, confidence, reason}`（原则 5 可解释）。

### 3.A Land Score（14 项，按用户公式）

```
score_land =
   w1  · landcover_entropy              // Shannon 熵 H(land cover 分数)，归一
 + w2  · dominant_class_impurity        // 1 - max_class_fraction
 + w3  · normalized_lai_variability     // norm(std(LAI)) [现状 ✅]
 + w4  · normalized_elevation_range     // norm(max-min DEM) [缺失]
 + w5  · normalized_slope_variability   // norm(std(slope)) [现状 ✅]
 + w6  · topographic_wetness_importance // norm(TWI = ln(a/tanβ)) [缺失]
 + w7  · river_distance_importance      // exp(-d_river / L_r) [MERIT 已有数据]
 + w8  · soil_texture_heterogeneity     // norm(std(soil hydraulic/thermal)) [现状 🟡 ks/ksol/tk*]
 + w9  · groundwater_sensitivity        // norm(|∇ water table depth|) [缺失]
 + w10 · snow_permafrost_priority       // max(snow_cover_freq, permafrost_prob) [缺失]
 + w11 · urban_priority                 // impervious_fraction [缺失]
 + w12 · cropland_irrigation_priority   // cropland_frac (+ irrigation 加权) [缺失]
 + w13 · climate_gradient_priority      // norm(|∇P| 或 aridity 偏离) [缺失]
 + w14 · user_defined_priority          // 用户掩膜/区域 0..1
```

### 3.B Ocean Score（12 项，按用户公式）

```
score_ocean =
   w1  · coastline_complexity           // 单元内岸线分形/曲率 [缺失]
 + w2  · exp(-distance_to_coast / Lc)   // 近岸优先 [缺失]
 + w3  · normalized_bathymetry_gradient // norm(seaslope) [现状 ✅]
 + w4  · shelf_break_priority           // |∂depth/∂x| 在陆架坡折带峰值 [缺失]
 + w5  · estuary_priority               // CaMa is_estuary→1 [数据已有, 未接]
 + w6  · river_mouth_priority           // CaMa reach mouth [数据已有, 未接]
 + w7  · tidal_channel_priority         // 潮汐通道掩膜 [缺失]
 + w8  · normalized_sst_gradient        // norm(|∇SST|) [现状 ✅ sst m/s]
 + w9  · normalized_eke                 // norm(EKE) [现状 ✅]
 + w10 · sea_ice_edge_priority          // 海冰边缘带 [缺失]
 + w11 · narrow_strait_priority         // 窄海峡连通保持 [缺失]
 + w12 · island_preservation_priority   // 小岛/群岛保真 [缺失]
 // 可选附加项：w13 · normalized_ssh_gradient  [现状 ✅ ssh m/s，用户清单第13项]
```

### 3.C Atmosphere Score（10 项，按用户公式）

```
score_atmos =
   w1  · normalized_topographic_gradient   // norm(|∇elev|) [缺失]
 + w2  · orographic_precipitation_priority // 迎风坡 × 降水 [缺失]
 + w3  · extreme_precipitation_frequency   // P99 频率 [缺失]
 + w4  · tropical_cyclone_track_density     // TC 路径密度 [现状 ✅ typhoon]
 + w5  · storm_track_density                // 温带气旋路径密度 [缺失]
 + w6  · convective_frequency               // 对流频率 [缺失]
 + w7  · land_sea_contrast                  // |T_land - T_sea| / 岸线带 [缺失]
 + w8  · urban_heat_priority                // 城市热岛 [缺失]
 + w9  · sst_front_priority                 // norm(|∇SST|)（复用 ocean SST）[缺失]
 + w10 · user_priority                      // 用户掩膜 0..1
```

---

## 4. Preset Table（8 Land + 5 Ocean + 6 Atmos = 19）

权重 0–1（相对强度，0=关闭）。每个 preset = 一组 `CriterionConfig` 默认权重（[03 `MeshIntentPreset`](./03_config_schema_audit.md#23-11-种-mesh-意图--preset-映射)）。

### 4.A Land presets（列 = w1..w14）

| Preset | w1 entropy | w2 impurity | w3 LAI | w4 elev | w5 slope | w6 TWI | w7 river | w8 soil | w9 GW | w10 snow/PF | w11 urban | w12 crop | w13 climate | w14 user |
|--------|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| General balanced | .5 | .4 | .5 | .5 | .5 | .3 | .3 | .4 | .2 | .3 | .3 | .3 | .3 | 0 |
| Hydrology-focused | .3 | .2 | .3 | .4 | .6 | **.9** | **.9** | .5 | **.7** | .2 | 0 | .3 | .3 | 0 |
| Carbon-cycle | **.8** | **.7** | **.9** | .3 | .3 | .2 | .2 | .5 | .2 | .2 | .2 | **.6** | .4 | 0 |
| Snow/permafrost | .3 | .2 | .3 | **.8** | **.7** | .2 | .2 | .4 | .3 | **1.0** | 0 | 0 | .5 | 0 |
| Urban-focused | .5 | .4 | .3 | .3 | .3 | .2 | .3 | .2 | .2 | .1 | **1.0** | .3 | .3 | 0 |
| Agriculture/irrigation | .5 | .4 | **.7** | .2 | .3 | .5 | .5 | .5 | **.6** | .1 | .2 | **1.0** | .4 | 0 |
| Mountain terrain | .3 | .2 | .3 | **1.0** | **1.0** | .5 | .3 | .3 | .2 | .5 | 0 | .1 | .4 | 0 |
| Land-atmosphere coupling | .5 | .4 | **.7** | .4 | .4 | .4 | .3 | .4 | .3 | .3 | .3 | .4 | **.8** | 0 |

### 4.B Ocean presets（列 = w1..w12）

| Preset | w1 coast cplx | w2 dist-coast | w3 bathy grad | w4 shelf | w5 estuary | w6 rivermouth | w7 tidal | w8 SST | w9 EKE | w10 ice | w11 strait | w12 island |
|--------|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| Coastal ocean | **.9** | **1.0** | .6 | .4 | .6 | .5 | .6 | .3 | .2 | 0 | **.7** | **.8** |
| Shelf sea | .6 | .7 | **.9** | **1.0** | .5 | .4 | .5 | .4 | .3 | 0 | .5 | .5 |
| Estuary | **.8** | **.9** | .5 | .3 | **1.0** | **1.0** | **.9** | .2 | .1 | 0 | .6 | .5 |
| Global ocean balanced | .3 | .3 | .6 | .4 | .2 | .2 | .1 | **.8** | **.9** | .5 | .3 | .3 |
| Storm-surge/coastal-risk | **.9** | **1.0** | .5 | .5 | .7 | .6 | **.8** | .2 | .2 | 0 | .6 | .6 |

### 4.C Atmosphere presets（列 = w1..w10）

| Preset | w1 topo grad | w2 orographic | w3 extreme P | w4 TC | w5 storm | w6 convect | w7 land-sea | w8 urban heat | w9 SST front | w10 user |
|--------|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| Global atmosphere balanced | .6 | .5 | .5 | .5 | .5 | .4 | .4 | .2 | .4 | 0 |
| Typhoon-focused | .3 | .4 | **.7** | **1.0** | .5 | .5 | .4 | .1 | **.6** | 0 |
| Orographic precipitation | **1.0** | **1.0** | .6 | .2 | .3 | .4 | .3 | .1 | .2 | 0 |
| Regional climate downscaling | **.8** | **.7** | .6 | .4 | .5 | .5 | **.6** | .3 | .4 | 0 |
| Urban climate | .3 | .3 | .5 | .2 | .2 | .4 | **.6** | **1.0** | .2 | 0 |
| Land-atmosphere coupling | .5 | .5 | .5 | .3 | .4 | **.7** | **.9** | .3 | **.6** | 0 |

---

## 5. Required Datasets

| Score term | 数据集（示例） | 变量 | 现状 | 接入路径 |
|------------|----------------|------|------|----------|
| landcover entropy / impurity | IGBP/USGS/MODIS landtype | class fractions | landtype 已读 | 由计数→熵/纯度 |
| LAI variability/seasonality | MODIS/Copernicus LAI clim | LAI 月场 | ✅ m/s | 加 seasonality |
| elevation range / slope / curvature / topo grad | MERIT-DEM / SRTM / GMTED | elevation | DEM(MERIT elv 已读) | 接入 score |
| TWI / drainage / river distance/density | MERIT-Hydro (upa/dir/wth) | upa,dir,wth | 已读([02](./02_workflow_consistency_audit.md)§5) | **低垂果实** |
| soil texture/hydraulic/thermal | SoilGrids / GSDE / CoLM soil | ks,θ,tk* | ✅ 部分 | 已有 |
| groundwater | Fan et al. WTD | water table depth | ❌ | 新数据层 |
| snow / permafrost / glacier | MODIS snow / permafrost prob / RGI | freq,prob,frac | ❌ | 新数据层 |
| urban/impervious/cropland/irrigation | GHSL / GLC / GMIA | fractions | ❌ | 新数据层 |
| precip gradient / aridity / extreme P | CHIRPS/ERA5/GPCP clim | P, P99 | ❌ | 新数据层 |
| bathymetry depth/slope/shelf | GEBCO / ETOPO | depth | 🟡 seaslope | 加深度/坡折 |
| coastline complexity / distance-to-coast | GSHHG / OSM coastline | polyline | ❌ | 新数据层 |
| estuary / river mouth / discharge | CaMa-Flood | is_estuary,reach Q | 数据已读未接 | **低垂果实** |
| tidal range/channel | FES/TPXO | amplitude | ❌ | 新数据层 |
| SST/SSH/EKE | OISST / AVISO | fields | ✅ | 已有 |
| sea ice edge | NSIDC | concentration | ❌ | 新数据层 |
| TC / storm track / convective | IBTrACS / 再分析 | track density | 🟡 typhoon | 泛化 |
| land-sea contrast / urban heat / SST front | ERA5 / LST | T fields | ❌ | 新数据层 |

> 原则：每个 score term **必须**声明 `physical_process` 与 `CriterionDataSource`（[03](./03_config_schema_audit.md#3-rust-type-sketches)），缺数据则该 term 退化为 0 并在报告中记录（不静默细化）。

---

## 6. GUI Recommendation（文案 + 控件，对接 `CriterionGuiSpec`）

呈现遵循 [03 三档渐进式](./03_config_schema_audit.md#6-gui-mapping)：Guided 选 preset；Standard 调单 term；Expert 改归一化/数据层。每个 term 的 UI 由 `CriterionGuiSpec` 自动渲染。示例文案（label / help / 单位 / 默认）：

| Term | GUI label | help（为什么细化，原则 5） | unit | 默认权重 |
|------|-----------|-----------------------------|------|----------|
| landcover_entropy | "Land cover diversity" | "单元内地表类型越混杂，地表通量越异质，需更细网格" | – | 0.5 |
| normalized_lai_variability | "Vegetation (LAI) variability" | "植被密度空间变化大处影响蒸散/碳通量" | m²/m² | 0.5 |
| normalized_elevation_range | "Terrain elevation range" | "高差大处影响温度/降水/能量分布" | m | 0.5 |
| topographic_wetness_importance | "Wetness / valleys (TWI)" | "汇流低洼带影响土壤水文与径流" | – | 0.3 |
| river_distance_importance | "Proximity to rivers" | "近河道处水文过程梯度大" | km | 0.3 |
| snow_permafrost_priority | "Snow / permafrost" | "雪盖/冻土边界影响地表能量与碳" | frac/prob | preset |
| exp(-dist_to_coast/Lc) | "Coastal proximity" | "近岸处陆海交互与浅水动力强" | km | preset |
| coastline_complexity | "Coastline complexity" | "曲折岸线需高分辨率以保真" | – | preset |
| estuary_priority | "Estuaries / river mouths" | "河口盐淡水混合与通量集中" | – | preset |
| normalized_sst_gradient | "SST fronts" | "海表温度锋面驱动中尺度过程" | K/100km | 0.5 |
| tropical_cyclone_track_density | "Typhoon / TC tracks" | "台风高发路径需细化以解析强对流" | tracks | preset |
| orographic_precipitation_priority | "Orographic precipitation" | "迎风坡地形抬升降水梯度大" | – | preset |

GUI 必备（补 [02](./02_workflow_consistency_audit.md)§10 缺口）：
- **每单元 score 热力图** + 点击显示 `reason`（哪个 term 贡献最大）→ 原则 5。
- **before/after 质量卡片**（§7 指标）→ 原则 4/5。
- preset 下拉 + "解释此 preset"（列出启用 term 与权重）。

---

## 7. Quality Metrics（细化质量评估，对接 `CriterionQualityContribution`）

| 域 | 质量指标 | 定义 | 门禁建议（`QualityConstraintConfig`） |
|----|----------|------|----------------------------------------|
| 通用几何 | min angle / aspect ratio / well-centered | 三角/六边形质量 | min_angle ≥ 25°（Block） |
| 通用数值 | min edge / max refine ratio | 最小边长(CFL)、相邻层级比 | min_edge ≥ L_CFL；ratio ≤ 2（Block/Warn） |
| 通用过渡 | transition smoothness | 细化带梯度平滑度 | 无突变（Warn） |
| Land | landcover boundary fidelity | 细化是否对齐 landtype 边界 | 覆盖率 ≥ 阈值 |
| Land | LAI/slope variance resolved | 细化后子格方差下降比 | 下降 ≥ X% |
| Land | river connectivity | 河网在网格上连通 | 无断流（Block，hydro preset） |
| Ocean | coastline fidelity (Hausdorff) | 网格岸线 vs 真实岸线距离 | ≤ 阈值 |
| Ocean | bathymetry gradient resolved | 坡度被解析比例 | ≥ X% |
| Ocean | estuary/channel resolution | 河口/海峡最小宽度单元数 | ≥ N cells |
| Atmos | orographic gradient resolved | 地形梯度被解析比例 | ≥ X% |
| Atmos | track-band coverage | TC/storm 带细化覆盖率 | ≥ X% |
| 耦合 | land/ocean fraction 守恒 | Σ fraction=1（见 [02](./02_workflow_consistency_audit.md) W1/W2） | err ≤ 1e-6（Block） |

> 核心改进：每个 score term 的 `quality_contribution` 声明它 `improves`（如 RiverConnectivity）与 `may_degrade`（如 MinEdge↓→CFL 风险）的指标 → 把"细化收益 vs 质量代价"显式化（原则 3/4）。

---

## 8. Priority Roadmap

| 优先级 | 内容 | 理由 | 依赖 |
|--------|------|------|------|
| **P0 — 框架化现有判据（低风险高回报）** | 把现状 ✅ 变量（LAI/slope/soil/SST/SSH/EKE/seaslope/typhoon）封装为 plugin criteria，接入 `score_*` 归一化+权重+preset；加 `RefinementBudget` | 不引入新数据即满足原则 2/3/5；行为可与现状回归比对 | [03 PATCH-S1/S2/S3](./03_config_schema_audit.md#10-patch-plan) |
| **P0 — 接入已读未用数据（低垂果实）** | 把 MERIT-Hydro(elv/upa/dir/wth) → elevation/TWI/river-distance/drainage；CaMa(is_estuary) → estuary/river-mouth | 数据已在项目读取([02](./02_workflow_consistency_audit.md)§3/§5)，仅需接 score | MERIT/CaMa reader 已存在 |
| **P1 — 海岸/陆海守恒** | coastline complexity / distance-to-coast / 守恒 fraction（接 `overlay_cell`，[02](./02_workflow_consistency_audit.md) W2） | 修 coupling Blocker；coastal/estuary preset 必需 | geometry overlay 已实现 |
| **P1 — 高价值新数据层** | urban/impervious、snow/permafrost、bathymetry depth/shelf、orographic precip + topo grad（atmos） | 覆盖最常用 preset（urban/snow/shelf/orographic） | 新 DataLayer |
| **P2 — 气候/极端/动力** | precip gradient/aridity/extreme P、storm track、land-sea contrast、sea-ice edge、tidal | 进阶 preset | 气候再分析数据 |
| **P3 — 稀缺/高级** | groundwater、ET sensitivity、Rossby radius、jet stream、aerosols、population、mountain wave、LAI seasonality | 数据稀缺或专用 | 专项数据 |

> 落地顺序与 [03 Patch Plan](./03_config_schema_audit.md#10-patch-plan) 对齐：先 S1/S2/S3 打通 plugin+score 框架，再按 P0→P3 增量加 criterion，每个 criterion 独立 PR + 单测 + 质量贡献声明。

---

## 关键证据索引（file:line）

- 现状判据：`core/lib.rs:1256-1275`（`th_*`/`refine_*` 数组）；GUI 映射 `gui/main.rs:3613-3707`（land one/two-layer、ocean、atmos typhoon）
- 已读未用数据：MERIT-Hydro `cli/lib.rs:731/889`、CaMa `is_estuary` `cli/lib.rs:4571`（见 [02](./02_workflow_consistency_audit.md)§3/§5）
- 守恒缺口：`geometry/lib.rs:114-210`（overlay 未被 coupling 调用，[02](./02_workflow_consistency_audit.md) W2）
- plugin/score 落点：[03 §3 `RefinementCriterion`](./03_config_schema_audit.md#3-rust-type-sketches) + `RefinementBudget` + `QualityConstraintConfig`

*本报告为物理细化设计提案；所有现状结论基于实际源码字段。未修改任何 `src/rust` 代码。score 公式遵循用户给定骨架并补全归一化/数据/质量定义。*

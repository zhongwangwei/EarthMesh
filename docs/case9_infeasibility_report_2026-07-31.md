# Case 9 不可满足性报告（原生 15″ 全球海洋三角形）

日期：2026-07-31
仓库基线：`b7694d4a4b40b4ab1c460127926019777fc12ee2`
输入：`input/landtype_igbp_update.nc`
SHA256 `89bde86be2436f8762bd9d2b9bcfa727193e74299941e9d1545222b54e41be2a`

> **这不是机器可验证的 UNSAT 证明。** 它是一份带穷举证据的**不可满足性报告**：对若干
> **明确界定的有限域**给出 `0 SAT` 的穷举结果，并明列哪些域**未**穷举。结论相对于
> 下述 scope 成立，不可外推。

---

## 1. Scope（结论的适用边界）

| 维度 | 取值 |
|---|---|
| 需求源 | 原生 `86400×43200` 15 arc-second IGBP landcover，**禁止降采样与 coarse projection** |
| 拓扑后端 | 球面 Method-C（stride-3 种子格、`rad3` 足迹、`mrow` 过渡行） |
| 模板族 | 当前 canonical split 表；`perim_fill3_method_c` 固定 rewiring |
| 量化器 | `level = log2(h_base / h)`，`base_m = 32,947.83 m`（NXP=243） |
| 基础分辨率 | NXP = 81 / 162 / 243 |
| 层级上限 | `max_level = 3` |
| 硬需求语义 | 欠覆盖即失败（不降级、不删除、不粗投影） |

**结论只在以上全部条件同时成立时有效。** 任一条改变（换模板族、改粒度、放宽硬覆盖、
提高 NXP 至未测值）都需重新评估。

---

## 2. 结论

在上述 scope 下，**未找到任何合法赋值**；对下列有限域的穷举均为 `0 SAT`。

失败恒定地终止于同一处（NXP=243）：

```
Current nested grid 3 crosses the parent boundary in Method-C transition
at W face 517469 (mrlw=1, lon=-71.974, lat=-52.473)
```

---

## 3. 已穷举的有限域

| 域 | 规模 | 结果 | 证据 |
|---|---:|---|---|
| 固定 phase、canonical 一圈的 seed 子集 | `1,048,576` | `0 SAT` | `cluster-3-ring-1-fixed-phase-aggregate.json` |
| 周界分量独立 triple offset | `3^10 = 59,049` | **`PATCH_UNSAT`** | `perimeter-component-offset-full-{a,b}.json` |
| 单组件 coherent phase（component 12 的 6 相位） | `6` | `0 SAT` | `component-phase-sweep.json` |
| 单组件 phase variants（几何锚点跨层复验） | `14` | `0 SAT` | `phase-anchor-support-sweep.json`、`single-component-phase-probe.json` |
| 簇 3 基线 seed 域 | `256` | `0 SAT` | `cluster-3-dump-{a,b}.json` |
| NXP=162 簇 1 完整枚举 | `65,536` | `0 SAT` | `cluster-1-dump-a.json` |
| 单步改善动作的完备二元组合 | `4,560` | `0 SAT` | `improving-toggle-pairs-{a,b}.json` |
| 24-face 依赖域单/双/三步组合 | `469` | `0 SAT` | 见评审文档 §7 |

其中**唯一够格称为 `PATCH_UNSAT`** 的是周界 triple offset 域：域明确、穷尽、可重放。
其余因边界越界或域未完备，严格状态为 `INCOMPLETE`。

> **溯源说明**：§3 与 §4 的枚举结果记录在
> `docs/method_c_case9_legalization_tasklist_2026-07-29.md`（该文件保存 Case 9 的逐次
> 实验与证据哈希）；§5–§7 及四开关部分记录在
> `docs/method_c_high_speed_exact_legalization_research_2026-07-30.md` §12–§32。

---

## 4. 已否证的解释

| 假设 | 否证依据 |
|---|---|
| hard demand 没有合法 canonical placement | `0/112` unsupported——每个 hard face 都有足迹可覆盖 |
| 硬需求约束过紧、退让少量即可通过 | 覆盖松弛下 `conceded = 0`，否决从未触发 |
| 存在遗漏的 template placement 自由度 | emitter 审计：`emit_method_c_tables` 离散输入仅有掩码、有序周界、固定表 |
| 「全选」是合法上界，只增闭包必可达 | 全选 `2470/2470` legal seeds 在 preflight 即失败（non-triplet） |
| 需求正则化可解 | 统一膨胀（`0/7`）、单分量加圈（`0/90`）、形态学闭运算（半径 5 转 Valence），三条全反证 |
| 提高 `max_level` 可解 | `max_level=2` 与 `=3` 失败周界逐字相同；`=4` 结构性不可能（第 4 层在失败时尚不存在） |
| 提高 NXP 可解 | `81 -> 162 -> 243` 各指标单调下降但未清零，且边际递减 |
| 按 hard-coverage 分量分解可行 | 单分量 `2^65`；且漏掉 `26` 个 topology-only seeds |
| 存在紧凑充分统计量（现有字段） | 全 symbolic 字段压缩仅 `1.25x` 且仍不充分 |
| 逐个绕过约束可解 | 四个默认关闭开关全开后失败面逐字不变 |

---

## 5. 未穷举的域（结论不覆盖这些）

1. **联合 phase × seed 域**——按当前 checkpoint 划分为 `6 × 2^9 = 3072`，全流水线枚举约
   `217 h`，且父层重建后 component/phase 划分会变，非稳定静态域；
2. **NXP=243 最大耦合分量**——`241` 变量（NXP=81）/ `34` 变量（NXP=243，`1.7e10` 赋值）；
3. **簇 3 二圈及更远**——`26` 变量起，`6.4` 天单线程；
4. **NXP > 243**；
5. **`max_level` 与 NXP 的联合扫描**。

---

## 6. 能恢复可行性的最小改动（已实测）

| 改动 | 效果 | 代价 |
|---|---|---|
| `max_level = 1` | ✅ `840,025` 单元，全门 `pass`，`81 s` | **需求同时被削掉**（全部单元目标层级为 0），不是「满足了 15″ 需求」 |
| 按分量退让不合格周界 | Method-C 合法化 **93%** 的细化（退 `82/1181` 面） | 制造 `76` 个 lineage 的新支撑缺口，外溢大于退让量；仍未通过 |

**没有任何已测改动能在保持 scope 全部条件的前提下恢复可行性。**

---

## 7. 会改变结论的方向（未实施）

1. **扩充 canonical transition 模板**——从失败边界签名出发离线枚举更细 split；
2. **合法化翻边**——启动条件（「证明合法标记在现有模板下不可达」）已接近满足；
3. **细化粒度下沉**——分级细化实测最大价数 `6`，落在 `[usize; 7]` 内；接入点在
   **gridfile 之后**（`dimc` 动态、拓扑硬门接受价数 `12`），不在 emitter 内。
   **但中间步骤未验证**：需求驱动二分能否满足 15″ hard demand 尚无证据，因为真实需求
   没有产物留存——成功运行（`max_level=1`）的需求为空，失败运行不写 artifact
   （见研究文档 §33）；
4. **按拓扑族分流生成器**——tri 产品不需要 TRiSK 对偶。

---

## 8. 交付语义

对使用方的正确表述是：

> **在当前 Method-C 模板族、`max_level = 3`、NXP ≤ 243 与当前量化规则下，
> 该 15″ 需求不可满足。** 已穷举的有限域见 §3；未穷举的见 §5。
> 这不是「不存在任何可行网格」，而是「在此规格下不可行」。

不得据此删除、降采样或粗投影任何 15″ hard obligation。

---

## 附：溯源

- **枚举与穷举证据**：`docs/method_c_case9_legalization_tasklist_2026-07-29.md`
  （§3、§4 的全部数字与证据哈希出自此处）
- **求解器路线与四开关记录**：
  `docs/method_c_high_speed_exact_legalization_research_2026-07-30.md` §12–§32
- 外部算法图谱：`docs/mesh_algorithm_landscape_survey_2026-07-29.md`
- 证据目录：`target/case9-*`、`target/mesh-refinement-m0-*`

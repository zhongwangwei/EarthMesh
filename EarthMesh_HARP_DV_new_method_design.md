# EarthMesh-HARP-DV：层级自适应细分–重定位球面 Delaunay–Voronoi 方法

## 1. 方法定位

**HARP-DV** 的英文全称为：

> **Hierarchical Adaptive Refinement and Positioning for spherical Delaunay–Voronoi meshes**

中文名称：

> **层级自适应细分–重定位球面 Delaunay–Voronoi 方法**

该方法面向 EarthMesh 的实际约束设计：

- 土地利用、地形、LAI、土壤、海洋动力等阈值触发自动细化；
- 指定点、半径、走廊、闭合区域细化；
- 支持连续多次局部细化，不依赖 Method-C 的 stride-3、rad3、mrow 周界；
- 未变化区域保持原坐标、原单元 ID 和原连接关系；
- 新增单元具有父子谱系，可生成局地保守重映射；
- 最终同时得到球面 Delaunay 三角网和 Voronoi 多边形网；
- 兼容 MPAS、FVCOM、CoLM 和 EarthMesh 现有质量检查、掩膜和耦合流程；
- 可使用纯 Rust 实现，不依赖外部闭源或特殊许可网格库。

HARP-DV 不是宣称在所有场景下替代 JIGSAW-GEO。JIGSAW-GEO 仍然是成熟的一次性高质量球面网格生成器。HARP-DV 的优势集中在 **已有网格上的增量修改、单元谱系、远场不变、阈值闭环和局地保守映射**。

---

## 2. 为什么 EarthMesh 需要不同于 JIGSAW-GEO 的方法

连续尺寸场 + JIGSAW-GEO 的基本管线是：

```text
多源判据
   ↓
连续目标尺寸场 h(x)
   ↓
全局或大区域重新生成网格
   ↓
Delaunay / Voronoi 输出
```

该方法适合一次性生成最终网格，但对 EarthMesh 还存在以下结构性不足：

1. 重新生成后，原单元 ID、父子关系和远场连接通常不能保持；
2. 一个小范围阈值变化也可能导致大范围网格变化；
3. 需要重新构造旧网格到新网格的全局映射；
4. 阈值细化必须先间接转成规则栅格上的尺寸场；
5. 土地利用这种分类异质性会受到尺寸场栅格分辨率和插值方式影响；
6. 难以表达“先尝试移动现有节点，仍不能满足阈值时再增加节点”的自适应逻辑；
7. 不适合反复执行“评估—细化—再评估”的闭环。

HARP-DV 改为：

```text
当前 Voronoi 单元
   ↓
直接计算单元内部的阈值违反程度和误差位置
   ↓
局地移动现有生成点（r-adaptation）
   ↓ 仍不满足
在单元内部插入新的生成点（h-adaptation）
   ↓
局地恢复球面 Delaunay + 局地 CVT 优化
   ↓
只更新局部网格、谱系和重映射
```

因此，HARP-DV 不要求先构造全局 HField。现有 HField 仍可作为一种可选判据输入，但不再是唯一入口。

---

## 3. 核心数据结构

HARP-DV 以 Voronoi 生成点作为稳定单元身份。Delaunay 三角形是生成点集合的连接结构，Voronoi 多边形是其对偶。

```rust
pub struct AdaptiveSite {
    pub id: CellId,
    pub parent: Option<CellId>,
    pub generation: u8,
    pub xyz: [f64; 3],
    pub original_xyz: [f64; 3],
    pub frozen: bool,
    pub trigger_mask: CriterionMask,
    pub created_iteration: u32,
}

pub struct DemandEvidence {
    pub criterion_id: CriterionId,
    pub statistic: f64,
    pub threshold: f64,
    pub normalized_violation: f64,
    pub confidence: f64,
    pub requested_scale_m: f64,
    pub witness: Option<[f64; 3]>,
}

pub struct CellDemand {
    pub cell_id: CellId,
    pub actual_scale_m: f64,
    pub requested_scale_m: f64,
    pub violation: f64,
    pub predicted_benefit: f64,
    pub evidences: Vec<DemandEvidence>,
    pub preferred_witness: Option<[f64; 3]>,
}
```

关键约定：

- 原始生成点默认永不删除；
- 新生成点是某个当前 Voronoi 单元的 child；
- 原始及已有生成点的 ID 始终不变；
- 新 ID 可由 `parent_id + generation + child_ordinal` 确定性生成；
- Delaunay 三角形可以局地变化，但 Voronoi 单元身份绑定生成点，而不是绑定易变化的三角面编号；
- 每次网格修改以 transaction 执行，质量失败时可完整回滚。

---

## 4. 判据不再只输出 0/1，而是输出“证据 + 目标尺度 + 误差位置”

统一接口：

```rust
pub trait RefinementCriterion {
    fn evaluate(
        &self,
        cell: &SphericalPolygon,
        site: &AdaptiveSite,
        data: &DataContext,
    ) -> Result<DemandEvidence>;
}
```

每个判据返回：

1. 当前单元统计量；
2. 用户阈值；
3. 阈值违反程度；
4. 数据可信度；
5. 建议目标尺度；
6. 最应增加分辨率的位置 `witness`；
7. 可审计的触发原因。

### 4.1 连续变量通用规则

对于统计量 `s` 和阈值 `τ`：

\[
q = \max\left(0, \frac{s}{\tau}-1\right)
\]

建议层级增量：

\[
\Delta \ell =
\min\left(
L_{\max}-\ell,
\left\lceil \log_2\left(1+q\right) \right\rceil
\right)
\]

目标尺度：

\[
h^* = \frac{h_{\mathrm{current}}}{2^{\Delta \ell}}
\]

可增加死区与迟滞：

- refine：`s > τ_refine`；
- keep：`τ_coarsen ≤ s ≤ τ_refine`；
- coarsen：`s < τ_coarsen`；
- 要求 `τ_coarsen < τ_refine`，避免反复振荡。

### 4.2 土地利用判据

分类数据不应只用均值和标准差。对一个 Voronoi 单元内的类别比例 `p_c`，计算：

类别数：

\[
N_c = |\{c:p_c>0\}|
\]

主导类别不纯度：

\[
I_d = 1-\max_c p_c
\]

归一化 Shannon 熵：

\[
H = -\frac{\sum_c p_c\ln p_c}{\ln N_c}
\]

类别边界密度：

\[
B = \frac{\text{单元内不同类别像元邻接边总长度}}{\text{单元面积}}
\]

综合统计量：

\[
s_{\mathrm{lc}}=
\max(w_NN_c^*,\;w_II_d,\;w_HH,\;w_BB^*)
\]

`witness` 优先放在：

- 类别边界密度最高的位置；
- 少数类别斑块中心；
- 与已有生成点距离较远的位置。

### 4.3 地形判据

对单元内 DEM 拟合局地平面或二次曲面 `z_hat(x)`：

\[
E_{\mathrm{dem}} =
\sqrt{
\frac{1}{A_C}
\int_C [z(x)-\hat z(x)]^2\,dA
}
\]

并可组合：

- 高程极差；
- 坡度均值和标准差；
- 最大坡度；
- 曲率；
- ruggedness；
- 局地拟合残差。

`witness` 取残差最大且与现有生成点保持足够距离的样本位置。

### 4.4 指定点和半径

对于点 `p`、核心半径 `r`、过渡宽度 `t`：

\[
w(d)=\frac{1}{2}
\left[1-\tanh\left(\frac{d-r}{t}\right)\right]
\]

\[
h^*(d)=h_{\mathrm{fine}}
+(h_{\mathrm{coarse}}-h_{\mathrm{fine}})[1-w(d)]
\]

其中 `d` 为球面大圆距离。

该判据可直接求值，不需要先栅格化。若点落在一个较大的 Voronoi 单元中，`witness` 就是该点或点在单元内的最近可行位置。

### 4.5 多判据合成

每个判据输出一个 `h_k^*`，最终目标尺度取：

\[
h^*(C)=\min_k h_k^*(C)
\]

同时保留所有触发来源，不再只保留最终的最小值。

综合违反程度：

\[
v(C)=\max\left(0,\frac{h_{\mathrm{actual}}(C)}{h^*(C)}-1\right)
\]

---

## 5. 自适应循环

```text
Evaluate
  ↓
Relocate existing sites locally
  ↓
Re-evaluate
  ↓
Mark cells that still violate thresholds
  ↓
Predict split benefit and forced-balance cost
  ↓
Select non-overlapping refinement patches
  ↓
Insert child sites
  ↓
Restore spherical Delaunay locally
  ↓
Apply graph-balance closure
  ↓
Patch-local CVT / ODT optimisation
  ↓
Quality gate + accept or rollback
  ↓
Repeat until converged or budget reached
```

---

## 6. R-step：先移动现有生成点，再决定是否增加单元

JIGSAW-GEO 主要解决重新生成问题；HARP-DV 首先尝试局部 r-adaptation。

对违反阈值的单元及其 2–3 环邻域：

1. 计算判据权重密度 `ρ(x)`；
2. 计算每个生成点对应单元的加权球面质心；
3. 将生成点向质心移动，但限制最大位移：

\[
d_g(p_i^{new},p_i^{old})
\le \eta_r h_i,
\qquad \eta_r\approx0.1\text{--}0.25
\]

4. 外围一圈生成点冻结；
5. 每次移动后执行局地 Delaunay edge flip；
6. 若移动已经使阈值满足，则不增加单元。

这一步有三个作用：

- 将现有分辨率重新分配到高需求位置；
- 降低新增单元数量；
- 避免单纯 h-refinement 在边界附近产生过密过渡带。

---

## 7. H-step：局地插入新的 Voronoi 生成点

### 7.1 子点必须位于父 Voronoi 单元内部

父单元定义清楚以后：

- 新 site 的 `parent_id` 唯一；
- 局地保守重映射只需计算父单元及邻域；
- 远场单元完全不变；
- 父子谱系不依赖 Delaunay 三角形编号。

### 7.2 候选点不是固定用三角形中心

构造三个候选：

1. **feature witness**：判据误差最大处；
2. **weighted farthest point**：

\[
p_f=\arg\max_{x\in C}
\rho(x)\,d_g(x,V)^2
\]

3. **spherical off-center**：在最欠采样 Delaunay 边的球面垂直平分方向上，按目标尺度放置。

候选得分：

\[
J(p)=
\alpha\,\hat\rho(p)
+\beta\frac{d_g(p,V)}{h^*}
-\gamma P_{shape}(p)
-\delta P_{boundary}(p)
\]

只接受满足以下条件的候选：

- 位于父 Voronoi 单元内；
- 与已有生成点距离不小于 `η_s h*`；
- 预测插入后最小角不低于安全下限；
- 不跨越固定边界或特征线；
- 不产生退化球面三角形。

### 7.3 球面 Bowyer–Watson 插入

对选定新点：

1. 查找其 Delaunay cavity；
2. 删除 cavity 内三角面；
3. 提取唯一有序 cavity 边界环；
4. 连接新点与边界环；
5. 更新 M/U/W 互反邻接；
6. 验证 Euler 特征、边二重性、顶点扇连通性和球面朝向。

整个操作只修改一个连通局部 patch，不产生 hanging node。

---

## 8. 连续图平衡：替代 Method-C 的固定过渡模板

HARP-DV 不要求固定层级周界，而在当前 Delaunay 邻接图上执行 balance closure。

定义单元有效尺度：

\[
s_i=\sqrt{\frac{A_i}{\pi}}
\]

对任意相邻 Voronoi 单元 `i,j`，要求：

\[
\frac{\max(s_i,s_j)}{\min(s_i,s_j)}\le q_{\max}
\]

建议：

- 保守模式：`q_max = sqrt(2)`；
- 通用模式：`q_max = 2`；
- 专家模式：用户指定。

也可同时约束逻辑代数差：

\[
|g_i-g_j|\le 1
\]

出现违反时，不修改深层区域边界，而是把较粗单元加入 forced-refinement queue。该闭包与树形 AMR 的 2:1 balance 思想相似，但作用于任意球面 Delaunay 邻接图。

这一步替代：

- 人工 HALO；
- stride-3 phase；
- rad3 footprint；
- mrow 周界行；
- 第二层必须完全落在父层纯内部的限制。

---

## 9. Patch-local CVT/ODT 优化

插点后只优化局地 patch，而不是重新优化全球网格。

建议目标函数：

\[
E(P)=
 w_c E_{cvt}
+w_a E_{area}
+w_q E_{quality}
+w_d E_{displacement}
+w_f E_{feature}
\]

其中：

### 密度加权 CVT 项

\[
E_{cvt}=\sum_i\int_{C_i}
\rho(x)d_g(x,p_i)^2\,dA
\]

### 目标面积项

\[
E_{area}=\sum_i
\left[\log\frac{A_i}{A_i^*}\right]^2
\]

### 三角形质量项

\[
E_{quality}=\sum_T
\psi(\alpha_{min}(T))
+\lambda_r\psi(R_T/l_{min,T})
\]

### 原始生成点位移惩罚

\[
E_{displacement}=\sum_{i\in old}
\frac{d_g(p_i,p_i^0)^2}{h_i^{*2}}
\]

### 特征约束项

固定海岸、河网、区域边界上的生成点不得离开约束曲线；邻近特征点可沿曲线切向移动。

MVP 阶段不必立即实现完整 L-BFGS。可以先实现：

```text
受限球面 Lloyd step
→ 投影回球面
→ 局地 edge flip
→ 质量检查
→ 阻尼或回滚
```

---

## 10. 预测收益与单元预算

不是所有超过阈值的单元都应立即细化。

对一个候选父单元，用当前 site 和候选 child 做一次便宜的两中心样本划分，估计分裂后的误差：

\[
\widehat E_{after}
\]

预测收益：

\[
B_i=E_{before}-\widehat E_{after}
\]

考虑 balance closure 预计额外产生的单元数：

\[
C_i=1+N_{forced,i}
\]

优先级：

\[
P_i=\frac{c_i B_i}{C_i}
\]

其中 `c_i` 为数据置信度。

按照 `P_i` 排序并在 `max_cells`、`max_added_cells_per_pass` 和内存预算下选择。这样可以避免少量噪声阈值导致全球级联细化。

---

## 11. 批量并行

同一批次只选择 cavity 或 2–3 环 patch 不相交的单元。

```text
候选单元图
  ↓
按 CellId 确定性排序
  ↓
构造 maximal independent set
  ↓
并行生成局部 transaction
  ↓
确定性顺序提交
```

初版应先实现单线程、固定顺序和位级确定性，再增加并行。

---

## 12. 局地保守映射

每次 transaction 保存修改前后的局部 Voronoi 多边形。

对旧单元 `i` 和新单元 `j` 计算球面交叠面积：

\[
W_{ij}=\frac{|C_i^{old}\cap C_j^{new}|}{|C_i^{old}|}
\]

只需要处理 transaction patch 内的单元。远场单元直接使用单位映射：

\[
W_{ii}=1
\]

要求：

\[
\sum_j W_{ij}=1
\]

并输出：

- `parent_cell_id`；
- `old_to_new_overlap`；
- `new_to_old_overlap`；
- `created_by_criterion`；
- `adapt_iteration`；
- `far_field_unchanged`。

---

## 13. 可选反向粗化

对新插入且没有后代的 leaf site：

1. 当前所有判据都低于 coarsen 阈值；
2. 删除后不会违反图平衡；
3. 局地 cavity 可合法重三角化；
4. 质量门控通过；
5. 保守映射可构造；

则允许删除。

原始 base sites 默认不可删除，从而保留基础网格骨架。

---

## 14. 必须维持的不变量

### 拓扑不变量

- 全球球面网格 `V-E+F=2`；
- 每条活跃 Delaunay 边恰有两个相邻三角形；
- 无非流形边；
- 每个顶点扇连通；
- 三角面绕向一致；
- Delaunay/Voronoi 邻接互逆。

### 几何不变量

- 所有 site 位于规定球面半径；
- 无重复或过近 site；
- 无零面积或非有限三角形；
- 最小角、边长比和面积比通过质量门控；
- 固定特征点仍在其约束上。

### 谱系不变量

- 旧 site ID 不变；
- 每个新 site 只有一个 parent；
- parent 必须在创建前存在；
- transaction 失败不得留下部分谱系；
- 远场坐标与连接关系逐位不变。

### 数值不变量

- 同输入、同线程设置、同排序策略得到相同结果；
- 采用稳健球面朝向和 cavity 判定；
- 几何接近退化时使用确定性 tie-break，而不是随机扰动。

---

## 15. 与 JIGSAW-GEO 的比较

| 能力 | JIGSAW-GEO | HARP-DV |
|---|---|---|
| 一次性生成高质量球面网格 | 强 | 初期不如成熟 JIGSAW |
| 直接使用连续尺寸场 | 强 | 支持但非必需 |
| 直接使用单元阈值 | 需先转尺寸场 | 原生 |
| 土地利用分类异质性 | 需栅格化映射 | 直接在单元内计算 |
| 指定点和半径 | 通过尺寸函数 | 解析判据 + witness |
| 增量修改已有网格 | 非主要目标 | 核心能力 |
| 原单元 ID 保持 | 通常不保持 | 保持 |
| 父子谱系 | 不提供 | 提供 |
| 远场逐位不变 | 不保证 | 设计要求 |
| 局地保守映射 | 需全局计算 | transaction 内局地计算 |
| 反复评估—细化闭环 | 需要反复重生成 | 原生 |
| 局地粗化 | 通常重新生成 | 叶节点删除 |
| 纯 Rust / EarthMesh 自有后端 | 否 | 是 |
| 成熟度与理论质量保证 | 高 | 尚需实现和验证 |
| 复杂边界支持 | 成熟 | 后续阶段实现 |

因此，HARP-DV 的“更合适”是针对 EarthMesh 的增量工作流，而不是宣称其一次性网格质量已经超过 JIGSAW-GEO。

---

## 16. EarthMesh 代码架构建议

当前：

```text
earthmesh_hfield
    ↓
earthmesh_mesh / Method-C
```

建议：

```text
earthmesh_criteria
    ↓
earthmesh_adapt
    ├── marking + budget
    ├── lineage
    ├── transactions
    └── balance closure
          ↓
earthmesh_mesh
    ├── method_c          compatibility backend
    ├── harp_dv           new backend
    ├── spherical_delaunay
    └── local_cvt
          ↓
earthmesh_quality / writers / coupling
```

### 后端接口

```rust
pub trait AdaptiveMeshBackend {
    fn adapt(
        &self,
        mesh: &CanonicalPrimalDualMesh,
        criteria: &[Box<dyn RefinementCriterion>],
        policy: &AdaptPolicy,
        data: &DataContext,
    ) -> Result<AdaptOutcome>;
}
```

### Project YAML 示例

```yaml
refinement:
  enabled: true
  backend: HarpDv

  policy:
    max_cells: 2000000
    max_added_cells_per_pass: 50000
    max_iterations: 20
    balance_ratio: 2.0
    relocation_fraction: 0.20
    optimisation_rings: 3
    min_angle_deg: 25.0
    deterministic: true

  criteria:
    - type: LandCoverEntropy
      threshold: 0.35
      coarsen_threshold: 0.20
      max_generation: 4

    - type: TerrainResidual
      threshold_m: 50.0
      coarsen_threshold_m: 25.0
      max_generation: 5

    - type: PointRadius
      lon: 120.0
      lat: 40.0
      radius_km: 500.0
      transition_km: 300.0
      target_km: 10.0
```

现有 HField 可保留：

```yaml
  demand_mode: DirectCellOracle   # 默认
  # demand_mode: HField           # 兼容/对照
```

---

## 17. 分阶段实现

### 阶段 A：最小可验证原型

范围：

- 全球球面；
- 现有 icosahedron Delaunay parent；
- point-radius 判据；
- 单点插入；
- 球面 cavity 更新；
- site ID 与 parent ID；
- Voronoi 重建；
- Euler、互反邻接、最小角质量检查；
- 单线程确定性。

这一阶段不做 CVT、不做粗化、不做复杂边界。

### 阶段 B：直接阈值判据

增加：

- 土地利用 entropy / impurity / boundary density；
- DEM residual / slope / range；
- witness 选择；
- benefit/cost 标记；
- graph balance closure；
- 多个不相连细化区。

### 阶段 C：局地 r-adaptation 和 CVT

增加：

- 受限 Lloyd；
- patch 冻结环；
- edge flip；
- transaction rollback；
- 面积和目标尺度优化。

### 阶段 D：保守映射与粗化

增加：

- 局地球面 polygon overlap；
- parent/child remap；
- leaf site 删除；
- refinement restart。

### 阶段 E：区域和特征约束

增加：

- 区域闭合边界；
- 海岸线约束；
- 河网/走廊约束；
- 边界点切向优化；
- 区域 Euler 与孔洞拓扑。

### 阶段 F：并行

增加：

- 不相交 patch 选择；
- 并行 demand evaluation；
- 并行 transaction 构建；
- 确定性 commit；
- MPI 分区和 ghost patch。

---

## 18. 回归测试矩阵

### 几何基础

- 全球均匀网格插入一个点；
- 北极附近插入；
- 日界线附近插入；
- 十二个五边形附近插入；
- 多次在同一父单元附近插入；
- 多个不相交 patch；
- 近退化 cavity；
- 固定随机顺序和倒序输入结果一致。

### 指定区域

- 单点 + 半径；
- 两个重叠圆；
- 多个分离圆；
- 圆跨日界线；
- 圆靠近极区；
- 走廊；
- 细化需求从弱到强连续增加。

### 土地利用

- 两类直线边界；
- 棋盘格；
- 小孤岛类别；
- 多类别碎片；
- 全球真实土地利用数据；
- 不同源分辨率和缺测值。

### 地形

- 平面：不应细化；
- 单高斯山：山峰局地细化；
- 山脊：沿山脊细化；
- 陡坡 + 平原；
- 多尺度真实 DEM；
- 噪声 DEM：预算和置信度不得导致失控。

### 谱系与守恒

- 旧 site ID 全部保留；
- 每个 child 唯一 parent；
- far-field 坐标逐位相同；
- overlap 行和等于 1；
- 常量场重映射后严格保持；
- 面积加权总量保持。

---

## 19. 与 Method-C 和 JIGSAW-GEO 的正式对照指标

### 需求满足

- 请求尺度与实际尺度比；
- 超阈值单元剩余比例；
- witness 周围分辨率；
- 土地利用/DEM 表示误差下降；
- 指定区域漏细化面积。

### 网格质量

- 最小角和角度分布；
- radius-edge ratio；
- 单元面积 CV；
- 邻接面积比；
- well-centered 比例；
- Voronoi 紧致度；
- 拓扑错误数。

### 稳定性

- 多次重复 byte-diff；
- 远场改变单元比例；
- 原始 ID 保留率；
- 局地重映射非零元数量；
- 增量运行与从头重建结果差异。

### 效率

- 每新增单元耗时；
- demand evaluation 耗时；
- cavity 平均大小；
- 优化 patch 大小；
- 峰值内存；
- 并行加速比。

---

## 20. Go / No-Go 验收条件

HARP-DV 进入生产后端前至少满足：

1. 所有全球测试均保持 `V-E+F=2`；
2. 无非流形边、断裂顶点扇和无效索引；
3. 指定点/半径可连续达到至少五次等效细化，不出现 Method-C 类父层边界失败；
4. 细化 patch 外的 site 坐标和连接关系保持不变；
5. 新 site 的 parent 与触发判据可完整追溯；
6. 同输入重复运行得到相同输出；
7. 所有修改均经过 transaction，质量失败可回滚；
8. 常量场的局地保守重映射残差接近机器精度；
9. 至少在 point-radius、土地利用和地形三个代表案例中，以相近单元数达到不低于 Method-C 的质量和明显更高的需求满足率；
10. 与 JIGSAW-GEO 的比较必须诚实报告：若一次性全局网格质量或速度不及 JIGSAW，则保留 JIGSAW 作为 benchmark/fallback。

---

## 21. 方法创新性判断

HARP-DV 的单个组件并非全部首次出现：

- 自适应误差标记来自有限元自适应思想；
- 局地 Delaunay 插点已有成熟理论；
- 球面 Delaunay 和 Voronoi 已有算法；
- CVT、ODT 和球面最优输运已有研究；
- 2:1 balance 在树形 AMR 中很成熟。

潜在原创贡献是它们在 EarthMesh 约束下的组合：

1. **直接阈值到当前球面 Voronoi 单元的 demand oracle**，不强制经过固定规则 HField；
2. **以生成点 ID 为核心的谱系保持局地 Delaunay 细化**；
3. **连续 Delaunay 图平衡**，替代固定层级周界模板；
4. **先局地重定位、后局地插点的 h-r 联合策略**；
5. **局地保守重映射和远场逐位不变**；
6. **每个新单元带有判据来源、置信度和 predicted benefit 的可解释记录**；
7. 同一个原始–对偶网格直接服务于 MPAS、FVCOM、CoLM 和水文/海岸耦合。

这些要点如果通过严格数值实验和复杂度分析，有潜力形成独立的方法论文。但在完成原型验证以前，应称为“新方法设计与研究假设”，不能预先宣称其质量或速度全面超过 JIGSAW-GEO。

---

## 22. 参考方法基础

- Engwirda, D. (2017). JIGSAW-GEO: locally orthogonal staggered unstructured grid generation for general circulation modelling on the sphere.
- Alkämper, M., Gaspoz, F., and Klöfkorn, R. (2018). A Weak Compatibility Condition for Newest Vertex Bisection in Any Dimension.
- Ju, L., Gunzburger, M., and Zhao, W. (2006). Adaptive Finite Element Methods Based on Conforming Centroidal Voronoi–Delaunay Triangulations.
- Weller, H., Browne, P., Budd, C., and Cullen, M. (2016). Mesh Adaptation on the Sphere using Optimal Transport.
- McRae, A. T. T., Cotter, C. J., and Budd, C. (2018). Optimal-Transport-Based Mesh Adaptivity on the Plane and Sphere.
- Prill, F. and Zängl, G. (2017). A Compact Parallel Algorithm for Spherical Delaunay Triangulations.
- Jacobsen, D. W. et al. (2013). Parallel Algorithms for Planar and Spherical Delaunay Construction with an Application to CVTs.
- Yang, H., Gunzburger, M., and Ju, L. (2018). Fast Spherical Centroidal Voronoi Mesh Generation.
- Burstedde, C. and Holke, J. (2016). A Tetrahedral Space-Filling Curve for Nonconforming Adaptive Meshes.
- Bonito, A., Nochetto, R. H., and Pauletti, M. S. (2010). Geometrically Consistent Mesh Modification.


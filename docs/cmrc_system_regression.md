# CMRC 共享流程与跨领域回归

## 一套内核，不把不同交付对象混为一谈

陆面、大气和海洋通过配置进入共同的 `run_refine_pipeline_namelist`。
启用 certified 后，共同使用 `run_certified_pipeline`、`certified_requirement_plan` 和
`build_mixed_certified_construction`，而不是按 L3、A3、O3 的名称切换算法。
这些入口位于 `rust/earthmesh_cli/src/refine_pipeline/global_source.rs`。
不细化的普通陆面控制案例仍走既有均匀生成路径；它们不能用来衡量 CMRC 粗化加速。
共享体系不等于所有场景强行使用相同参数：过渡圈数可以显式配置，回归必须按相同配置比较，
不能根据案例名在内核中偷偷改参数，也不能把不同圈数的控制组和选定交付方案混为一谈。

| 层次 | 共享职责 | 必须保留的差别 |
|---|---|---|
| 数据和判据 | 根据启用的变量读取、统计、校验；`area_judge_threshold_inputs`、`threshold_support` | 变量、存储值单位和阈值由数据/配置定义，支持与掩膜规则共用 |
| 分辨率需求 | 合成区域与阈值需求，限制渐变，再投影至母网格 | 原始命中、有效栅格、图调度、最终单元是不同对象，不能混算 |
| CMRC | 在全球母网格上进行受约束粗化 | NXP 必须属于已验证的母网格层级族；不按案例名称准入 |
| 全球大气交付 | `publish_certified_atmos_mpas` | 闭球面拓扑、MPAS 几何/度量/密度与 graph 一致性 |
| 区域海洋交付 | `publish_certified_domain_gridfile` | 全球粗化后取区域三角形，另查区域边界和 FVCOM 输出 |
| 区域陆面交付 | `cmrc_land::publish_regional_land` | 全球粗化后取完整对偶单元，另查多边形拓扑、绕序和血缘 |

全球母网格认证不自动覆盖区域交付。区域输出没有闭球面 Euler=2 要求；
三角形的 38–82° 窗口也不能直接施加到六边形内角。
区域 certified CoLM 路径目前交付的是原生网格文件及单元血缘，
不等于完整 CoLM 运行数据包，更不等于模型求解验证。区域保守映射仍单独标为不可用。

## 统一阈值语义：候选实现会改变科学结果

`refinement_demand/threshold_support.rs` 是 HField 和源索引需求规划的共同入口。
两条适配器只投影同一份逐级命中，不再分别计算点值 mean、一次性 std 或 HField 类别块。
陆面、海洋和大气变量使用同一统计规则；变量与阈值由既有配置目录决定。

- **独立统计支持**：每级父尺度为 `base_m / 2^(level-1)`；纬向格数为
  `ceil(πR / parent_m)`，经向为两倍，至少 4×2，最多 16,777,216 个支持。
  超限报错，不静默钳住分辨率。统计范围可以小于默认 720×360 合成栅格的一格。
- **明确几何近似**：支持为等角矩形分区，不是等面积区域、测地圆或 v2 的实际单元。
  纬度从 −90° 起分区，经度中心从 −180° 起，日期变更线上的支持周期连接且不重复计数。
  报告实际角度宽度与名义父尺度；不将二者说成处处相等的物理边长。
- **原始数据与单位**：直接读取原始有限源样本，保留存储值尺度，不猜测 LAI/SST 单位。
  新入口只接受规则全球二维经纬栅格；坐标存在时验证间隔、纬度中心、单调性与周期唯一性，
  无坐标时使用明确的旧式全球中心约定。非均匀、区域伪全球或二维曲线坐标报错。
- **样本规则**：有效样本等权；总体方差除以 N。mean 至少 1 个样本，std 至少 2 个；
  空支持不触发，也不作最近邻回填。读取到的无效数值仍报错。landtype 的缺失/maxlc
  在统计前排除，包括类别比例的分母；类别 0 为海洋，正类别为陆地。
- **严格判据**：mean/std 严格大于阈值；陆地类别数严格大于配置值；最大陆地类别占比
  严格小于阈值（分母仅为有效陆地）；海洋占比严格位于两阈值之间（分母为有效海陆总量）。
- **域与掩膜**：输出域和计算细化的 degree-zero 掩膜选择支持中心。被选中支持保留完整
  样本和命中足迹，允许越过边界；最终区域提取另行决定交付范围。极薄区域可能没有支持
  中心，此时记录诊断；不能声称这一中心采样精确表示任意边界。
- **独立等级与硬需求**：每级独立评价，后一级未命中不撤销前一级命中，前一级安静也不
  阻止细尺度命中。等级不得超过 `max_iter_cal` 或适配器显式上限；不借用区域/水文的更高等级。
- **保守投影**：完整命中足迹投影至所有正面积相交的 HField 格或指定源索引窗口，包含跨线
  足迹。投影不重新统计、不再次用另一粗栅格掩膜裁切；随后合成渐变与母网格需求。
  单独启用的 coastline 仍是几何特征，不是第二套统计判据。

审计策略为 `per_level_source_support`。`raw_support` 记录支持尺寸、命中、有效/空/单样本数、
掩膜选择规则和原始命中图诊断摘要；`projected_hit_bins` 与等级直方图记录投影后的合成栅格，
不是原始支持数、渐变扩展数或最终网格数。诊断哈希不是密码学完整性证据；回归产物另以 SHA256 绑定。

这一候选取代旧的“一格下限 / one-shot / point mean”科学语义，不能当作结果不变的重构。
先运行入口一致性、非单调等级、空样本、跨线和投影反例，再比较九案例需求、质量、覆盖与耗时：

```sh
cargo test --offline -p earthmesh_cli --lib refinement_demand::threshold_support -- --test-threads=1
cargo test --offline -p earthmesh_cli --lib hfield_refine -- --test-threads=1
```

旧式点值/滑窗读取仍是低层兼容工具，不再是生产规划的另一套判据。源索引计划入口可对照验证，
但 Method-C 的自适应正需求圆圈物化仍有独立的暂停准入，不能据此宣称整个圆圈后端已交付。
真实 L3 使用已有 LAI；A/O 控制案例使用指定区域，真实 SST/台风阈值数据仍未作生产规模验证。

## 一个批量入口，显式外部数据

```sh
python3 -B scripts/run_cmrc_system_regression.py \
  --cli /absolute/path/earthmesh_cli \
  --manifest /absolute/path/suite.json \
  --output /absolute/path/new-run-directory \
  --threads 16 --timeout 900 --rss-limit-gib 48
```

POSIX 环境，需要 `ps`；不新增 Python 依赖。清单固定同一个二进制、输入文件和 namelist 的
SHA256。外部大数据不拷入源代码仓库。每个 case 显式声明输出要求，例如：

```json
{
  "binary_sha256": "<sha256>",
  "inputs": {"/absolute/path/landtype.nc": "<sha256>"},
  "cases": [{
    "name": "regional_land",
    "config": "/absolute/path/regional_land.nml",
    "config_sha256": "<sha256>",
    "outputs": ["gridfile*.nc4"],
    "description": "regional native land grid"
  }]
}
```

`name` 必须与 namelist 的 `NL%EXPNME` 相同；入口只重写输出目录和线程数，不改阈值和质量参数。
配置采用每行一个 `NL%base_dir` / `NL%openmp` / `NL%EXPNME` 赋值的受限格式，不是通用 namelist 编辑器。
输出目录必须全新；逐案例记录命令、摘要、耗时、内存采样、退出码和产物哈希。
九案例套件应包含 L1–L3、A1–A3、O1–O3，各案例的物理格式和认证范围写入清单，
执行器没有按这些名称切换算法的分支。

**入口成功只表示生成成功且必需产物存在，不表示几何或模型认证。**
静态质量、区域拓扑、MPAS/FVCOM 格式和单元血缘还必须分别验证。
历史 v2 时间可作参考，但不同覆盖范围/需求/交付工作量不能用来给粗化算法直接排名。

小型守护回归：

```sh
python3 -B -m unittest discover -s scripts -p test_cmrc_system_regression.py -v
cargo test --offline -p earthmesh_cli --lib hfield_refine::tests::std_ -- --test-threads=1
```

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
| 数据和判据 | 根据启用的变量读取、统计、校验；`area_judge_threshold_inputs`、`hfield_refine` | 变量、单位、空间统计支持及掩膜由配置/数据定义 |
| 分辨率需求 | 合成区域与阈值需求，限制渐变，再投影至母网格 | 原始命中、有效栅格、图调度、最终单元是不同对象，不能混算 |
| CMRC | 在全球母网格上进行受约束粗化 | NXP 必须属于已验证的母网格层级族；不按案例名称准入 |
| 全球大气交付 | `publish_certified_atmos_mpas` | 闭球面拓扑、MPAS 几何/度量/密度与 graph 一致性 |
| 区域海洋交付 | `publish_certified_domain_gridfile` | 全球粗化后取区域三角形，另查区域边界和 FVCOM 输出 |
| 区域陆面交付 | `cmrc_land::publish_regional_land` | 全球粗化后取完整对偶单元，另查多边形拓扑、绕序和血缘 |

全球母网格认证不自动覆盖区域交付。区域输出没有闭球面 Euler=2 要求；
三角形的 38–82° 窗口也不能直接施加到六边形内角。
区域 certified CoLM 路径目前交付的是原生网格文件及单元血缘，
不等于完整 CoLM 运行数据包，更不等于模型求解验证。区域保守映射仍单独标为不可用。

## 当前阈值语义：明确，不暗中改变

- HField 连续变量 mean/std：在当前 HField 栅格上统计，命中后一次请求目标等级；
  报告策略为 `one_shot_target_level`。
- 类别数、比例等内容判据：逐级评价对应父单元尺度的栅格块；
  报告策略为 `per_level_parent_blocks`。栅格块不是实际的当前网格单元。
- 另一条 `refinement_demand/plan.rs` 路径按每级单元尺度选取 std 邻域。
  因此“同一个 std 阈值”在两条路径上不保证同样的需求。
- 标量场取样和空间平均不是同一操作；均值与标准差都可能随统计支持范围改变。
  不能用“都是标准差”或“均值不随尺度变化”抹平这些差别。

后续统一尺度语义会改变科学结果，必须先规定支持范围、空样本、跨掩膜和跨日期变更线行为，
用细/粗尺度反例验证，再比较原始需求、过渡扩展和最终网格。不得为匹配 v2 单元数而
直接降低等级、取消已有硬需求、缩小保护带或放宽质量门槛。

### 统计尺度准入：先诊断，不伪装成已经统一

当前类别判据的块宽为 `max(1, round(parent_m / latitude_bin_m))`；
经纬方向采用同样的格数，最后一块截断，不跨越 ±180° 拼接。
因此“逐级调用”不保证“逐级缩小统计范围”，也不保证正方形角度范围或等面积支持。
例如 NXP=144 的默认基准尺度约 55.6 km，而 720×360 HField 的纬向格宽也是约 55.6 km：
第 1、2 级的父尺度分别约 55.6、27.8 km，但实际都使用一格。
合并已有格子的 `count/sum/sumsq` 无法恢复格内分布；只抽出一个统计函数不能解决这个问题。

阈值审计中各条目的 `support` 是**只读诊断**：

| 字段 | 含义 |
|---|---|
| `target_parent_scale_m` | 该目标等级的名义父尺度；连续变量仍按 one-shot 策略，不能据此推断它已逐级统计 |
| `latitude_bin_m` | 单格南北向弧长；不是该纬度下的东西向距离 |
| `latitude_span_degrees` / `longitude_span_degrees` | 名义块几何角度宽度的 `[最小, 最大]`，包含全局末尾截断块；不是有效样本或最近邻回填源的实际范围 |
| `requested_parent_below_latitude_bin` | 所需父尺度小于一个纬向格；仅忽略 1e-12 相对舍入误差，不改变块宽计算 |
| `same_block_as_previous_level` | 类别判据是否复用上一级块宽；第一级和 one-shot 连续变量为 `null`，而非假装重新统计 |
| `longitude_blocks_wrap` | 当前块不会跨日期变更线合并；源数据经度坐标可周期映射是另一回事 |

`support.kind` 明确标记名义格块；mean 的空格可能使用格外最近邻源，
`empty_source_bins_before_nearest_fallback` 记录回填前的空格数，不声称所有空格都回填成功。
这些字段不参与需求生成或质量判定，不能据此取消已有命中。块宽的四舍五入误差、
日期变更线截断、经纬尺度差异和栅格中心掩膜近似都仍存在。

### 下一行为实验的共享规格（尚未切换默认实现）

1. **分离三种网格**：源数据统计支持、需求合成/渐变栅格、最终网格单元不能混为一谈。
   首个候选复用 `refinement_demand/threshold.rs` 的源像元中心矩形窗口，
   按父尺度换算纬向半径；经度周期、纬度在两极截断。它是角度窗口近似，
   不冒充真实单元内统计，也不承诺与 v2 的当前单元支持相同。
2. **统计与边界**：使用有限有效源像元的等权总体统计（方差除以 N，不是 N−1），
   严格 `>` 比较。均值至少 1 个样本、标准差至少 2 个；无样本不生成需求。
   先按 landtype/maxlc 规则排除样本，再统计；域边界限定需求输出，
   不截掉窗口的域外邻域。读取的数据缺失/非有限仍须明确报错，不静默降低需求。
   跨日期变更线的窗口不得重复计数；跨全周的支持必须封顶去重或拒绝。
3. **逐级独立**：每个启用等级独立评价，不能以前一级未命中为理由跳过细尺度，
   也不能用细尺度未命中撤销已产生的粗尺度硬需求。合并时取各位置的最细目标。
4. **投影保持需求**：将命中源支持位置投影到需求栅格时，保留所有相交的命中像元，
   包括日期变更线与掩膜边界；然后才作渐变限制和母网格调度。
   原始命中、投影扩展、渐变扩展与最终单元数分别报告。
5. **必须显式验证的现状差异**：当前 HField mean 的空格可最近邻回填，std 空格/单样本
   数值为 0 后直接比较阈值；它没有统一的显式样本数门槛。当前 HField 在统计前裁剪域，
   而源窗口路径保留域外邻域。当前 point “mean”也不是上述空间均值。
   这些不能被隐藏在“统计重构”里；特别是负阈值下，零值不能代表“无需求”。

先以小型反例锁定统计、掩膜、周期边界与投影行为，再做单独的行为实验和跨领域比较。
不默认增大整个 HField；统计支持应能细于合成栅格，并有内存上限。
此阶段只增加诊断和反例，**没有宣称阈值科学语义已统一**。

可重复的支持范围反例（不需外部大数据）：

```sh
cargo test --offline -p earthmesh_cli --lib threshold_contract -- --test-threads=1
cargo test --offline -p earthmesh_cli --test refinement_demand_threshold -- --test-threads=1
```

现有 std-only 加速只跳过不参与标准差判定的空格均值回填；共享实现没有区域/案例特判。
回归将 LAI、SST、typhoon 三种变量分别放入六种支持的领域名称/别名，验证相同 std
命中和 std→mean 缓存行为。它们是合成的小型算法测试，不冒充真实海温/台风数据验证。

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

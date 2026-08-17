# 循环兜底上界不该扫全网格

## 1. 发现

`heavy` CI 作业自 `12078c4` 起从 9m52s 涨到 2 小时被杀。本机复现
`harp_dv_output_passes_the_mesh_quality_gate`(NXP=21,debug),按
CLAUDE.md §11.3 对进程采样,5 秒 3637 个样本:

```
balance_objective
  → MeshState::voronoi_cell_from
    → MeshState::triangle_fan_from
      → MeshState::triangle_count          3305 样本 = 90.9%
        → Filter(active_triangle_slots → is_triangle_active)
```

**九成的时间在数三角形。**

## 2. 原因

`mesh_voronoi/mod.rs:153`:

```rust
let limit = self.triangle_count() + 1;
```

`mesh_state/mod.rs:413`:

```rust
pub fn triangle_count(&self) -> usize {
    self.active_triangle_slots().count()      // O(F) 全槽位扫描
}
```

这个 `limit` 的**唯一用途**是循环跑飞时的兜底:走满就返回
`FanDidNotClose`。一个扇形是约 6 个三角形;为了决定它最多能走几步,
先把整张网格(此算例约 15,000 个槽位)数一遍。约 2,500 倍的纯开销,
每次目标函数求值付一次。

同样的写法还有两处:

- `mesh_insertion/mod.rs:195` — `let limit = self.triangle_count() + 1;`(定位走步)
- `mesh_flip/mod.rs:219` — `let limit = 16 * self.triangle_count() + 64;`(Lawson 翻边)

翻边那处的注释自己写着 "Generous, and a bound rather than a guess" ——
说明作者本就只要一个上界,不要精确值。

## 3. 修改

三处都换成 `self.triangles().len()`,即槽位数:

- 是切片长度,**O(1)**;
- 恒 **≥ 活跃三角形数**(墓碑只会让活跃数更少),所以任何原先能走完的
  合法循环仍然走得完;
- 兜底语义不变:跑飞仍然被截住,只是截在一个更大的步数上。

## 4. 为什么这是保行为的

三处的 `limit` 只决定 `for _ in 0..limit` 的迭代上限。

- **成功路径不受影响**:合法扇形/走步/翻边在远小于任一上界处就返回,
  上界变大不改变任何返回值。
- **失败路径的错误种类不变**:拓扑损坏时循环仍然走满并返回同一个错误
  变体(`FanDidNotClose` / `LocationWalkDidNotSettle` / 翻边上限)。
- **唯一可观测差异**:上述错误负载里的 `visited` 计数在损坏情形下会更大。
  全仓检索确认没有任何测试或生产代码读取该字段(`FanDidNotClose` 只在
  定义处、`Display` 实现处和抛出处各出现一次)。

因此交付网格逐位不变。这一点用现有的等价性测试与 CLI 质量门断言验证,
不靠推断。

## 5. 不改的一处

`mesh_insertion/mod.rs:620`:

```rust
if cavity.len() >= self.triangle_count() {
    return Err(InsertionError::CavitySwallowedTheMesh { ... });
}
```

这是**语义检查**而非循环上界:它问的是"空腔是否已经吞掉整张网格"。
换成槽位数会把阈值抬高,使检查更难触发——那是削弱一个安全网,不是优化。
它每次插点只调一次,留着。

## 6. 实测结果

| 算例 | 前 | 后 | 倍数 |
|---|---|---|---|
| `harp_dv_output_passes_the_mesh_quality_gate`(NXP=21,debug) | CI 上 2 小时被杀;本机 1h08m 只走到 30/48 遍 | **8m02s**,48 遍全跑完 | ≥13× |
| harp_dv 单元套件 | CI 上 500.06s | **23.56s** | 21× |
| `the_full_production_path_on_the_nxp_proxy`(NXP80,release) | 1935.2s | **207.1s** | **9.3×** |
| CI `fast` 作业 | 超时被杀 | 4m56s | — |
| CI `heavy` 作业 | 超时被杀(2 小时) | 25m31s | — |

**输出未变,且是逐字节的未变**:

- CLI 算例:旧版被停之前吐出的 76 行(15 个细化周期、低度数修复、冻结目标场
  角度诊断、30 个优化器遍的每个计数与六位小数指标)与新版逐字节相同。
- NXP80:全部 **203 行**逐字节相同,站点数、周期数、停止原因一致
  (70,685 / 100 / `MaximumCyclesReached`)。

## 7. 与"按收益退出"的关系

先前写的 `harp-dv-quality-optimiser-payoff-exit.md` 归因于优化器跑满 48 遍。
那个观察本身成立(NXP80 后 13 个 window 遍只买到 0.72% 的收益),但量级
远小于此处的三个数量级,且**会改变交付网格**。两者分开:

1. 本改动:纯性能,不改输出,先落地;
2. 收益退出:改行为,需预登记验收,视本改动之后是否仍有必要再定。

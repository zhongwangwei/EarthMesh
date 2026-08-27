# HARP-DV 生产归因 trace：PR20a 微规格

日期：2026-08-27
状态：PR20a 实施规格
范围：只建立可复现的生产归因链，不改变网格决策

## 1. 目的与非目标

PR20a 回答以下问题：

- 越界角在哪个 HARP 阶段首次出现、持续或消失；
- 最终越界角对应的稳定站点身份、degree、谱系和候选来源；
- 相同输入是否产生语义相同且排序稳定的证据。

PR20a **不**实现 degree-4 逐 trial 拒绝审计、二环 patch、cavity 搜索、delta cache、
StrictWindow 或新的放点算法。degree-4 site/trial 审计留给 PR20b。

## 2. 分层

`earthmesh_refine_harp_dv`：

- 定义无 serde 依赖的 typed trace event；
- 在实际控制流中发出事件；
- 保存生产 candidate ladder 的 `CandidateSource`；
- 不读取环境变量，不知道 JSON、文件路径或发布协议。

`earthmesh_cli`：

- 读取 `EARTHMESH_HARP_TRACE_JSONL`；
- 将 typed event 映射为 CLI 本地 serde DTO；
- 管理 JSONL、partial 文件及最终发布；
- 在 trace 失败时禁止进入最终 gridfile 输出。

HARP crate 不增加 `serde`、`serde_json` 或其他依赖。现有无 trace API 保持不变；
traced overload 使用一个可变 callback，不增加单实现的公共 observer 框架。

## 3. 启用协议

```text
EARTHMESH_HARP_TRACE_JSONL=/absolute/path/harp_trace.jsonl
```

- 未设置：完全关闭，不创建文件，也不运行额外 certifier 扫描；
- 已设置：路径必须为绝对路径，父目录必须存在；
- 正式目标已存在：在 HARP 执行前失败，绝不覆盖；
- 任一创建、序列化、写入、flush、sync 或发布错误：CLI 返回错误，不写最终 gridfile；
- 不承诺回滚已经修改的内存 mesh，也不删除运行前已经存在的 gridinit/tmpfile。

## 4. 七个 checkpoint

成功 trace 恰好包含七条 `stage_summary`：

| index | stage |
|---:|---|
| 0 | `input` |
| 1 | `post_refinement` |
| 2 | `post_initial_low_degree` |
| 3 | `post_eta` |
| 4 | `post_window` |
| 5 | `post_final_low_degree` |
| 6 | `final` |

阶段未执行时先写 `phase_skipped`，随后仍写对应 `stage_summary`；该 snapshot 可与前一阶段相同。
这同样适用于空 criteria 和质量优化器的早退路径。

每个阶段写完整的 `angle_violation` snapshot。PR20a 不在生产端计算 transition；消费者可按相邻
snapshot 离线计算 `new/resolved/persisted/kind_changed`，避免同时维护两套等价证据。

## 5. 稳定身份与排序

```rust
AngleKey {
    triangle_sites: [SiteId; 3], // 升序
    corner_site: SiteId,
}
```

Face slot、triangle index 和 vertex slot 不是跨阶段身份。每个阶段的 violation 按
`AngleKey`，再按 violation kind 排序。JSON 记录不得包含 wall-clock、PID、partial 文件名或绝对
路径；相同 fixture 的语义 JSONL 应逐字节一致。

每条 violation 至少保留：

- `AngleKey`、kind、球面角；
- corner degree、三顶点 degree 三元组；
- refinement depth、birth cycle、birth candidate source；
- realized/target scale ratio 及其 measurable 状态。

## 6. CandidateSource 不变量

- 继承站点：`birth_cycle == 0` 且 `birth_candidate_source == None`；
- production candidate ladder 成功插入：`birth_cycle > 0` 且
  `birth_candidate_source == Some(source)`；
- 候选拒绝或事务回滚：不产生 `AdaptiveSite`；
- 直接低层 `propose_site_for()`：允许 `None`，不为测试/API 伪造来源。

PR20a 的 `CandidateSource` 表示发起插入的 ladder candidate。受保护 segment 改写候选点的运行
不在本轮真实 IGBP 归因范围内，不新增 `Unknown` 或 segment override。

## 7. JSONL 记录与完整性

稳定记录类型：

- `run_header`
- `stage_summary`
- `angle_violation`
- `phase_skipped`
- `harp_run_end`

`run_header` 至少包含 schema version、backend 和理论 stage 数；`harp_run_end` 至少包含各事件
计数和实际 stage-summary 数。`harp_run_end` 只表示 HARP 计算和 trace 完整结束，不表示最终
NetCDF gridfile 已成功写出。

非有限浮点不得进入 JSON number；输出 `null` 并带 `measurable=false`。

## 8. 无覆盖发布

1. 在目标同目录用 `create_new` 创建唯一隐藏 partial；
2. 写入 header、事件和 `harp_run_end`；
3. `BufWriter::flush()`；
4. `File::sync_all()`；
5. 关闭 writer/file，之后不得再写；
6. `hard_link(partial, target)`，由目标目录项创建原子裁决 no-clobber；
7. 成功后尽力删除 partial，删除失败只警告；
8. 然后才允许后续 remap/gridfile 输出。

不得回退到普通 `rename`。文件系统不支持 hard link 时显式启用的 trace 失败关闭。未同步父目录时
不宣称抵抗突然掉电；本协议保证进程级完整性与不覆盖。

失败运行保留唯一 partial。缺少正式目标或 `harp_run_end` 的 partial 不得被当作成功证据。

## 9. 最小验证

1. 环境变量未设置时，不创建 trace，现有输出不变；
2. 相对路径被拒绝；
3. 已有正式 trace 不被覆盖，并在 HARP/最终输出前失败；
4. 成功 trace 恰好包含 S0-S6、header 和 run-end；
5. skipped phase 同时有 `phase_skipped` 和 summary；
6. violation 按 `AngleKey` 稳定排序，同 fixture 重跑语义字节一致；
7. callback/write/flush/publish 错误被传播，且不进入最终 gridfile writer；
8. inherited/source/rollback provenance 不变量成立；
9. HARP crate 依赖集合不增加。

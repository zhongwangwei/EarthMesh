# CMRC 窗口边界身份拒绝诊断（2026-10-21）

延续[窗口拒绝试验](cmrc_window_rejection_20261021.md)，本轮已定位真实组件127、目标层级1的拒绝原因：**候选安装移除了同一组1,230个物理区域边缘source IDs；人工切口没有丢失，候选core/custom parent及其fine descendants均完整覆盖。** 这是现有窗口边界身份保护的保守拒绝，不是已证实的生产正确性漏洞，也不构成放宽门禁的依据。Window移动认证仍未通过。

## 诊断与独立核验

以提交 `320d02d83f8401961d7e3f51bdd44bb67e667ebe` 的归档源码构建私有overlay，只在既有 `edge_scope` 失败处记录数据，原拒绝原因、Whole disposition、认证和回放门禁保持不变。继续复用上一轮已认证的四点坐标捕获文件（SHA256 `b9d74f8e348d80488f0145f6ac9e2b87b8f229903e112ab52815ac828bf21d01`），同一二进制顺序运行control和shadow，使用既有原生约90 m MERIT、LANDTYPE及需求参数。

记录包括安装前全部开放边界source、原始物理边缘、人工切口、安装后活动/非活动compact→source映射，以及实际 `edge_scope` 计数使用的全部映射。独立Python检查从原始集合重新计算 `lost = original_edge − active_sources`，核对source映射唯一性、门禁计数、保留集合和物理/切口分区；未知或重叠分类、重复映射和覆盖遗漏均拒绝验收。候选core/custom parent地址及其fine descendants分别与窗口选择集合、实际 `work.within` 比较。

| 指标 | 四圈窗口 | 零圈窄窗口 |
| --- | ---: | ---: |
| 选择parent数 | 127,609 | 120,154 |
| fine source面覆盖数 | 2,041,744 | 1,922,464 |
| 安装前开放边界source数 | 8,620 | 6,808 |
| 安装后保留的原边界source数 | 7,390 | 5,578 |
| 丢失物理边缘source数 | 1,230 | 1,230 |
| 丢失人工切口source数 | 0 | 0 |
| 安装后活动source数 | 106,956 | 68,380 |
| 记录的Euler（前→后） | −12→−12 | −25→−25 |

两种窗口丢失的1,230个IDs逐项相同，完整列表保存在验收JSON中。两者都覆盖115,762个core parents（1,852,192个fine faces）和1,938个custom parents（31,008个fine faces），缺失集合为空。原人工切口分别6,160和4,348个source，全部保留。安装后开放source计数与保留原边界计数相等；本轮未据此声称两个开放集合逐项等价。Euler来自实际网格诊断，并非独立拓扑认证，不能据其不变推断窗口可接受。

两种窗口仍在 `InstallDelta / Whole` 拒绝，原因 `the candidate reached the edge of its window`，弹性迭代和interval boxes为0。拒绝发生在四点移动注入之前，原事务与克隆前态完全相同。Whole路径继续通过四点回放及域外/超限负向门禁；两个负例均在三条合法记录之后拒绝第四条，没有部分更新。

## 结果、成本与验证

T7920构建作业 `codex-d3ff55b1cab94d4fba59282241a4573c` 成功、退出0：32项独立定向Rust测试通过，包含新增边界分区回归。release二进制SHA256为 `65d317e65c513624f04f083ec9964755714ee02ed116b62f6f4f5c3766d84774`。独立诊断脚本通过18类损坏证据和3类非法元组输入自检，并保留原Whole正负门禁检查。

实际作业 `codex-4aa89f3ec89c400c9524458fb7040515` 成功、退出0；两次CLI均退出0，独立验收 `errors=[]`、`window_equivalence=false`。control耗时227.783 s、峰值RSS 3,485,868 KiB；shadow耗时266.621 s、峰值RSS 4,560,048 KiB。耗时包括多个克隆工作网格和大量诊断输出，不是优化器采用后的生产性能。

最终两份NetCDF（网格及regional parent）严格比较无差异，全部组件epoch、coarsening和adaptivity记录完全一致。最终质量仍为warn，相邻尺度比最大2.02415885936、连续性告警3对；没有实际采用影子候选。生产源码、CI配置与alpha11发布均未修改。

证据目录为 `.omx/artifacts/cmrc-boundary-loss-20261021/`。完整124,361,233字节原始回放保留在实际作业的 `results/proof/replay-audit/replay.txt`，在T7920完成独立复算，没有在本地进行大证据解析。原始回放SHA256为 `226c82c34f05ef3183a4f590bad5894dde5b71facb002b046aa14d63fc0f58dd`；已取回验收JSON的SHA256为 `2ad2755ecef17de0e63096262cfded04ec75a6c9b16521aff2b902fb5d393ed9`。

## 解释与下一步

`Work::window` 保存安装前全部开放边界；`edge_scope` 要求这些source身份全部保留。因此物理边缘细点被候选移除也会触发Whole回退，扩大人工切口并不能直接解除本例的身份约束。现有Whole区域几何scope允许映射当前仍存在的物理边界source，并继续完整检查几何、边界及Euler等约束。两者契约不同，不能把Whole通过解释为Window通过。

下一项最小验证是让同一候选从私有紧凑窗口进入**实际Window→Whole回退分派器**，核对重建、重认证、预算和状态，再与直接Whole的克隆结果比较。本轮各影子案例分别执行，没有证明该候选的实际自动回退链路。窗口移动认证、真实采用后的后续行为和多案例成本仍未覆盖；优化器保持私有，边界保护保持不变。

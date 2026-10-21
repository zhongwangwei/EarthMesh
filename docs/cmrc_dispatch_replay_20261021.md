# CMRC 实际窗口回退分派验证（2026-10-21）

延续[边界身份拒绝诊断](cmrc_boundary_loss_20261021.md)，本轮已验证同一真实组件127、目标层级1候选的回退链路：**紧凑Window安装失败后，共享分派器只重建一次Whole Work，保持Changed认证模式；重认证报告、计数及提交后完整克隆状态与直接Whole Work执行一致。** 本轮没有启用生产优化器，也没有放宽边界保护。

## 验证方式

以 `8b523b6c030ce349963bed70d9cb8f6be6de26ee` 归档源码构建私有快照。为避免手写第二份回退代码造成自证，机械提取原 `certify!()` / Window→Whole分支为一个私有helper，普通事务和两种影子路径都调用同一实现。提取前逐字校验原分支，SHA256为 `ba3e5cc7caeef560884fa27e7689c9a601acb82f168d46c0efbd338017741ceb`；保留原回退条件、state.refresh、Work::whole、计时和测试计数，以及刷新失败映射。生产工作区源码未改。

在原候选成功、真实commit之前，从同一刷新后的事务前态建立两条独立路径：dispatch以四圈紧凑Work开始，direct直接以Whole Work开始。两者都使用原transition、warm start和剩余预算，不重新搜索拓扑。候选精确相等断言与指纹记录同时保留；沿用已认证四点坐标捕获及原域/位移上限门禁，两个负例仍需在三条合法记录之后安全拒绝第四条。

重要区别：Whole Work是工作范围，Changed是认证模式。原回退只替换Work，不把认证模式改成Whole。本例最终两条路径均返回 `CommitGeometry::Changed`，不是Window移动认证，也不是本轮新增Whole模式认证。

## 实际记录

| 项目 | Window回退路径 | 直接Whole Work |
| --- | ---: | ---: |
| 首次Work范围 | 四圈Window | Whole |
| 分派器认证调用次数 | 2 | 1 |
| Whole重建次数 | 1 | 0 |
| 进入时弹性迭代上限 | 256 | 256 |
| 进入时剩余interval boxes | 246,128,778,240 | 246,128,778,240 |
| 成功报告弹性迭代 | 172 | 172 |
| 成功报告interval boxes | 129,975 | 129,975 |
| 提交后顶点/面数 | 722,976 / 1,444,720 | 722,976 / 1,444,720 |

首次窗口失败为 `InstallDelta / Whole`，原因为 `the candidate reached the edge of its window`，迭代和interval boxes均为0，未进入移动注入。随后真实共享分支执行 `state.refresh → Work::whole → certify`，第二次接受。两次调用与direct的候选指纹均为 `7656597464856972581`，fine source指纹为 `17030675104279238178`，warm-start指纹为 `5752548472714560345`；输入预算记录逐项一致。本例首次失败未产生需计入的工作量，不将此结果外推到所有晚期失败预算情形。

两条路径的完整报告Debug内容相等，commit并refresh后的完整 `ComponentTransactionState` 通过PartialEq比较；网格指纹均为 `12910772701636690303`，source等级哈希均为 `5441663457767268514`。原事务状态、候选和外层计数均保持不变。独立脚本检查调用顺序、一次回退、零消耗拒绝、候选/预算身份、四点移动、正负门禁、报告及提交指纹一致，不能仅凭成功退出通过验收。

## 构建、真实数据与成本

T7920构建作业 `codex-2489fcddacfa4b489d1edb095dd5096b` 成功、退出0：33项独立定向Rust测试通过，包含两个新增分派器回归（窗口Whole disposition恰好重建一次；Whole Work或Retry disposition不触发重建）。release二进制SHA256为 `517ed7116fe7e70ad125834b3ae20f74aba50b180e640e53a08777b047b52272`。私有生成源码有两处unused_mut警告，未为格式/冗余mut重新构建；不据此声称lint零警告。

独立验收脚本通过完整回放fixture、10类跨路径损坏、13类分派记录损坏及既有认证门禁自检。实际作业 `codex-2b19a2a3475741c89c3e0dec33d54a04` 成功、退出0，使用相同原生约90 m MERIT、LANDTYPE和四点捕获文件；两次CLI均退出0，验收 `errors=[]`。

control耗时231.606 s、峰值RSS 3,482,124 KiB；shadow耗时264.325 s、峰值RSS 4,964,480 KiB。成本包含影子克隆、额外认证及诊断，不能解释为真实采用后的生产性能。最终网格和regional parent两份NetCDF严格比较无差异，全部组件epoch、coarsening和adaptivity记录一致；本轮control事务记录与上一轮control也相同。质量仍为warn，最大相邻尺度比2.02415885936，连续性告警3对。

证据位于 `.omx/artifacts/cmrc-dispatch-replay-20261021/`。本地重新读取已取回的4,439,542字节原始回放、捕获坐标和证书独立复算通过。回放SHA256为 `d72d352c67707ff67dcbaaf165879dbb6bfec415270d99e26e919ba0765678bb`，验收JSON SHA256为 `371515b28c0ddb7a11e7f6b93d6181ddfff607152b5f716dec6cc33647dbdc88`。

## 结论边界与后续

本例的保守边界拒绝能够正确转入Whole Work重认证，未发现该回退分派链路的新bug。证据是私有快照中与原代码等价的共享分支和克隆事务，不是生产优化器已经接入。四点影子结果仍未commit到真实调度器，最终输出与control相同是预期结果。

下一项接入验证应在私有快照中将已通过事务认证的候选交给正常commit，并继续运行后续调度和最终认证，检查后续失败是否安全处理；通过后再扩展到其他代表性90 m案例。Window移动认证、真实采用后的后续行为和多案例成本仍未验证；生产行为及alpha11发布保持不变。

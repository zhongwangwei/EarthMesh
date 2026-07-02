#!/usr/bin/env bash
# EarthMesh perf probe: release build + timed runs + flamegraph.
# 用法:  bash scripts/perf_probe.sh [namelist] [runs]      ← 不要加 sudo！
# 默认:  examples/default/atmosphere_hex_global.nml, 2 次计时
# 输出:  docs/perf/timing.txt 和 docs/perf/flamegraph.svg
set -uo pipefail
cd "$(dirname "$0")/.."

if [ "$(id -u)" -eq 0 ]; then
    echo "请不要用 sudo 运行本脚本（会把 target/ 变成 root 所有）；"
    echo "flamegraph 需要提权时会通过 --root 自行调用 sudo。"
    exit 1
fi

NML="${1:-examples/default/atmosphere_hex_global.nml}"
RUNS="${2:-2}"
SMOKE="examples/00_quickstart_n16.nml"
OUT_DIR="docs/perf"
mkdir -p "$OUT_DIR"
: > "$OUT_DIR/timing.txt" || { echo "docs/perf 不可写（上次 sudo 残留？）：sudo chown -R \"$USER\" docs cases rust mkgrd.x"; exit 1; }

echo "== 1/4 release 构建（本次加了 debug 符号，LTO 全量重建需要几分钟）=="
cargo build --manifest-path rust/earthmesh_cli/Cargo.toml --release || exit 1
# macOS 内核按 inode 缓存代码签名，原地覆盖已存在的二进制会导致其被 SIGKILL。
# 必须先删除再拷贝，换到新 inode。
rm -f ./mkgrd.x
cp rust/earthmesh_cli/target/release/earthmesh_cli ./mkgrd.x

echo "== 2/4 冒烟运行（quickstart 小算例，确认二进制正常启动）=="
if ! ./mkgrd.x "$SMOKE" --quiet; then
    echo "冒烟运行失败（退出码 $?）。请把上方报错原样发回。"
    exit 1
fi
echo "冒烟通过。"

echo "== 3/4 计时 ${RUNS} 次: ${NML} =="
echo "case: ${NML}  ($(date))" >> "$OUT_DIR/timing.txt"
for i in $(seq 1 "$RUNS"); do
    echo "-- run $i --" | tee -a "$OUT_DIR/timing.txt"
    { /usr/bin/time -p ./mkgrd.x "$NML" --quiet; } 2>&1 | tee -a "$OUT_DIR/timing.txt"
done
echo "计时汇总:" | tee -a "$OUT_DIR/timing.txt"
grep "^real" "$OUT_DIR/timing.txt"

echo "== 4/4 flamegraph（xctrace 后端，无需 sudo）=="
if ! rm -rf cargo-flamegraph.trace 2>/dev/null && [ -e cargo-flamegraph.trace ]; then
    echo "残留的 cargo-flamegraph.trace 无法删除（root 所有）。请先执行:"
    echo "    sudo rm -rf cargo-flamegraph.trace"
    echo "然后重跑本脚本。"
    exit 1
fi
if command -v cargo-flamegraph >/dev/null 2>&1; then
    cargo flamegraph \
        --manifest-path rust/earthmesh_cli/Cargo.toml --release \
        --output "$OUT_DIR/flamegraph.svg" \
        -- "$NML" --quiet \
    && echo "flamegraph 已保存: $OUT_DIR/flamegraph.svg" \
    || echo "flamegraph 失败——把上方报错发回给我。"
    rm -rf cargo-flamegraph.trace
else
    echo "未安装 cargo-flamegraph：先执行 cargo install flamegraph 再重跑本步。"
fi

echo "完成。docs/perf/ 下的 timing.txt 与 flamegraph.svg 留在仓库里即可。"

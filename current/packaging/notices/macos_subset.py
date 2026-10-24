#!/usr/bin/env python3
"""Offline Cargo graph and installed npm reachability evidence; never builds."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "packaging/notices"
TARGET = "aarch64-apple-darwin"
ROOTS = ["switcher-desktop", "switcher-helper", "switcher-core"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(arguments):
    return subprocess.run(arguments, cwd=ROOT, env={**os.environ, "RUSTUP_TOOLCHAIN": "stable"},
                          check=True, capture_output=True, text=True).stdout


def main():
    locks = [ROOT / "Cargo.lock", ROOT / "apps/desktop/package-lock.json"]
    before = {str(p.relative_to(ROOT)): digest(p) for p in locks}
    inventory = json.loads((OUT / "inventory.json").read_text())
    assert before == inventory["lock_sha256_after"], "Refresh full inventory for new locks first"
    corpus = {(p["ecosystem"], p["name"], p["version"]): p for p in inventory["packages"]}
    result = {"target": TARGET, "cargo_roots": ROOTS, "lock_sha256_before": before,
              "commands": [], "editions": {}}
    for edition in ["personal", "public"]:
        flags = ["--offline", "--locked", "--no-default-features", "--features",
                 f"switcher-desktop/{edition},switcher-desktop/custom-protocol"]
        metadata_cmd = ["cargo", "metadata", *flags, "--filter-platform", TARGET, "--format-version", "1"]
        tree_cmd = ["cargo", "tree", *flags, "--target", TARGET, "--edges", "normal,build",
                    "--prefix", "none", "--format", "{p}"]
        for package in ROOTS:
            tree_cmd.extend(["-p", package])
        result["commands"].extend([metadata_cmd, tree_cmd])
        metadata = json.loads(run(metadata_cmd))
        tree = run(tree_cmd).replace(str(ROOT), ".")
        (OUT / f"macos-arm64-{edition}-tree.txt").write_text(tree)
        normal_cmd = tree_cmd.copy()
        normal_cmd[normal_cmd.index("normal,build")] = "normal"
        result["commands"].append(normal_cmd)
        normal_tree = run(normal_cmd).replace(str(ROOT), ".")
        (OUT / f"macos-arm64-{edition}-normal-tree.txt").write_text(normal_tree)
        normal_keys = set()
        for line in normal_tree.splitlines():
            match = re.match(r"^(\S+) v(\S+)", line)
            if match:
                normal_keys.add(match.groups())
        # Retain graph evidence, not absolute cache paths or irrelevant package descriptions.
        compact = {"packages": [{k: p[k] for k in ["id", "name", "version", "source", "license"]}
                                for p in metadata["packages"]], "resolve": metadata["resolve"]}
        for package in compact["packages"]:
            package["id"] = package["id"].replace(str(ROOT), ".")
        compact = json.loads(json.dumps(compact).replace(str(ROOT), "."))
        (OUT / f"macos-arm64-{edition}-metadata.json").write_text(json.dumps(compact, indent=2) + "\n")
        effective = set()
        for line in tree.splitlines():
            match = re.match(r"^(\S+) v(\S+)", line)
            if match:
                effective.add(match.groups())
        selected = [corpus["cargo", name, version] for name, version in sorted(effective)
                    if ("cargo", name, version) in corpus]
        metadata_only = [{"name": p["name"], "version": p["version"]} for p in metadata["packages"]
                         if (p["name"], p["version"]) not in effective]
        result["editions"][edition] = {
            "metadata_packages_count": len(metadata["packages"]),
            "resolve_nodes_count": len(metadata["resolve"]["nodes"]),
            "resolve_node_features": {n["id"].replace(str(ROOT), "."): n["features"] for n in metadata["resolve"]["nodes"]},
            "tree_effective_all_count": len(effective), "third_party_count": len(selected),
            "normal_edge_third_party_count": sum((p["name"], p["version"]) in normal_keys for p in selected),
            "build_only_third_party": [p["name"] + "@" + p["version"] for p in selected if (p["name"], p["version"]) not in normal_keys],
            "third_party": selected, "metadata_only_not_effective": metadata_only,
            "missing_text": [p["name"] + "@" + p["version"] for p in selected if not p["texts"]],
            "upstream_notice_review_required": [p["name"] + "@" + p["version"] for p in selected if p.get("upstream_notice_review_required")],
            "unknown_license": [p["name"] + "@" + p["version"] for p in selected if p["license"] == "UNKNOWN"]}

    npm = json.loads(locks[1].read_text())["packages"]
    unresolved = []

    def resolve(base, name):
        parent = PurePosixPath(base)
        while True:
            candidate = str(parent / "node_modules" / name)
            if candidate in npm:
                return candidate
            if parent == PurePosixPath("."):
                return None
            parent = parent.parent

    def reachable(sections):
        seen, todo = set(), [("", name) for section in sections for name in npm[""].get(section, {})]
        while todo:
            base, name = todo.pop()
            path = resolve(base, name)
            if path is None:
                unresolved.append({"from": base or "root", "dependency": name})
                continue
            if path in seen:
                continue
            seen.add(path)
            for section in ["dependencies", "optionalDependencies", "peerDependencies"]:
                todo.extend((path, dep) for dep in npm[path].get(section, {}))
        rows = []
        for path in sorted(seen):
            name = path.rsplit("node_modules/", 1)[-1]
            package = npm[path]
            row = dict(corpus["npm", name, package["version"]])
            manifest = ROOT / "apps/desktop" / path / "package.json"
            row["installed_now"] = manifest.is_file()
            if manifest.is_file():
                local = json.loads(manifest.read_text())
                assert local["name"] == name and local["version"] == package["version"]
            rows.append(row)
        return rows

    production = reachable(["dependencies"])
    build = reachable(["dependencies", "devDependencies"])
    result["npm"] = {"method": "lock dependency/optional/peer graph; installed manifests version checked; no bundler analysis",
                     "production_reachable": production, "build_reachable": build,
                     "unresolved_edges": sorted({json.dumps(x, sort_keys=True) for x in unresolved})}
    after = {str(p.relative_to(ROOT)): digest(p) for p in locks}
    assert before == after, "Lock changed during collection"
    result["lock_sha256_after"] = after
    (OUT / "macos-arm64-subsets.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    lines = ["# macOS arm64 发行依赖许可子集\n",
             "本报告使用离线、锁定的 Cargo 查询。没有运行编译。全平台原始报告与许可正文保持独立。",
             "它确认发行配置的依赖图与现有正文覆盖，不确认最终二进制实际包含的代码，也不代表许可放行。\n",
             "## 查询边界\n",
             "1. 目标固定为 `aarch64-apple-darwin`。根包包括 desktop、helper、core（CLI/guardian）。",
             "2. 两版均使用 `--no-default-features`，分别启用 `switcher-desktop/personal` 或 `switcher-desktop/public`，并启用 Tauri release 使用的 `custom-protocol`。",
             "3. `metadata.packages` 是包元数据集合；`resolve.nodes` 是带 feature 的解析节点集合。两者数量相同不代表所有包进入发行依赖图。",
             "4. 本报告用 `cargo tree --edges normal,build` 的指定根包闭包作为有效发行构建图，排除 dev 边。它保留 build/proc-macro，尚未按最终链接结果删除构建工具。",
             "5. npm 使用锁文件的 dependency、optional 和 peer 边，再核对当前 node_modules。两个发行共用前端依赖；edition 编译消除范围没有在本次核验中测量。\n",
             "## Cargo 计数\n", "| 发行 | metadata.packages | resolve.nodes | 有效图含第一方 | 第三方 | 归档材料缺项 | 上游专属通知待核 | 标识 UNKNOWN |",
             "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for edition, data in result["editions"].items():
        lines.append(f"| {edition} | {data['metadata_packages_count']} | {data['resolve_nodes_count']} | {data['tree_effective_all_count']} | {data['third_party_count']} | {len(data['missing_text'])} | {len(data['upstream_notice_review_required'])} | {len(data['unknown_license'])} |")
    lines += ["\n另行查询 `--edges normal`，区分只经 build 边可达的包。normal 图仍可含 proc-macro，不能称作实际运行时打包清单。",
              "Cargo metadata 的 features 是解析节点观察值；它不会完整表达 resolver v2 的 host/target 编译单元拆分。\n",
              "| 发行 | normal 边第三方 | 仅 build 图新增第三方 |", "|---|---:|---:|"]
    for edition, data in result["editions"].items():
        lines.append(f"| {edition} | {data['normal_edge_third_party_count']} | {len(data['build_only_third_party'])} |")
    personal = set(result["editions"]["personal"]["missing_text"])
    public = set(result["editions"]["public"]["missing_text"])
    lines += ["\n### 有效图剩余 Cargo 归档材料缺项\n", "| 包与版本 | 适用发行 | license 标识 | 缺项原因 |", "|---|---|---|---|"]
    for key in sorted(personal | public):
        name, version = key.rsplit("@", 1)
        row = corpus["cargo", name, version]
        editions = ", ".join(e for e, items in [("personal", personal), ("public", public)] if key in items)
        lines.append(f"| {key} | {editions} | {row['license']} | {row['text_status']} |")
    if not personal and not public:
        lines.append("| 无 | personal, public | — | 全部已有归档条款或声明；上游专属通知缺口另列 |")
    lines += ["\n## npm 可达与当前安装状态\n", "| 图 | 锁定可达包 | 当前已安装 | 已安装材料缺项 | 已安装上游通知待核 | 未安装可达包 |", "|---|---:|---:|---:|---:|---:|"]
    for label, rows in [("production（含 optional/peer）", production), ("build（含 dev/optional/peer）", build)]:
        installed = [r for r in rows if r["installed_now"]]
        lines.append(f"| {label} | {len(rows)} | {len(installed)} | {sum(not r['texts'] for r in installed)} | {sum(bool(r.get('upstream_notice_review_required')) for r in installed)} | {len(rows)-len(installed)} |")
    lines += ["\n当前已安装、且在 build 图中可达的 npm 正文缺项：\n", "| 包 | 版本 | 图 | license |", "|---|---|---|---|"]
    prod_keys = {(r["name"], r["version"]) for r in production}
    for row in build:
        if row["installed_now"] and not row["texts"]:
            scope = "production + build" if (row["name"], row["version"]) in prod_keys else "build"
            lines.append(f"| {row['name']} | {row['version']} | {scope} | {row['license']} |")
    if not any(row["installed_now"] and not row["texts"] for row in build):
        lines.append("| 无 | — | production + build | 全部已有归档许可材料 |")
    lines += ["\n## 20 项原缺口的补充证据与剩余边界\n",
              "5 个 Cargo 包已取得精确 VCS 提交的项目 LICENSE，或取得锁定源码明确引用的 Mozilla MPL-2.0 全文。两个 npm 平台绑定采用同版本主包完整许可证；主包 tarball 已对锁文件 SRI 校验。",
              "另有 11 个 objc2 系列 crate，以及 is-reference、locate-character，只能取得精确提交的许可声明和官方标准条款。上游没有在这些提交提供项目专属版权通知。",
              "**这 13 项上游通知缺口仍未关闭。** 标准许可保留版权占位符。没有填写作者姓名、没有制造 LICENSE，也没有把标准条款当作法律放行。",
              "objc2 原始 LICENSE.md 另保留 Apple SDK 衍生条款适用性说明。该说明没有被删去或推断为已解决。\n",
              "| 生态 | 包与版本 | 原始许可声明/版本依据 | 剩余项 |", "|---|---|---|---|"]
    reviewed = {(r["ecosystem"], r["name"], r["version"]): r
                for rows in [result["editions"]["personal"]["third_party"], result["editions"]["public"]["third_party"], build]
                for r in rows if r.get("upstream_notice_review_required") and (r["ecosystem"] != "npm" or r["installed_now"])}
    for key, row in sorted(reviewed.items()):
        evidence = row["supplemental_evidence"]
        original = evidence["files"][0]
        lines.append(f"| {key[0]} | {key[1]}@{key[2]} | [exact commit]({original['source_url']}) | 上游项目专属版权通知缺失；标准条款保留占位符 |")
    lines += ["\n逐包来源 URL、精确版本/提交绑定、正文摘要与适用依据见 [supplements/sources.json](supplements/sources.json)。",
              "补充过程与剩余边界见 [supplements/README.md](supplements/README.md)。现有 20 项均有条款/声明材料，但只有 7 项取得完整项目许可证或无占位符的明确引用许可；不要合并成‘20 项通知均已完整’。\n"]
    lines += ["\n未安装的 optional 平台包保留在锁图中，但不计入当前 node_modules 子集。完整名单与不可解析边见 JSON。",
              "npm 前端生产图不等于产物代码清单；peer TypeScript 等工具可能在生产图可达，不能据此认定其运行时代码被打入。\n",
              "## 证据与复现\n",
              "完整子集条目、license、来源、正文 SHA256、解析 feature 与 metadata-only 差集见 [macos-arm64-subsets.json](macos-arm64-subsets.json)。",
              "每版保留 `macos-arm64-*-metadata.json` 与 `macos-arm64-*-tree.txt`。metadata 文件只裁剪非图信息，保留 package IDs、resolve nodes、依赖边和 features。",
              "运行 `python3 packaging/notices/macos_subset.py` 可复现。命令完整记录在 JSON；只调用 offline locked metadata/tree。",
              "正文覆盖引用现有 inventory.json，不补写、不推断任何缺失 LICENSE。全平台材料仍见 [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)。\n",
              "锁文件收集前后 SHA256 一致：\n", "| 文件 | SHA256 |", "|---|---|"]
    lines.extend(f"| `{path}` | `{value}` |" for path, value in before.items())
    (OUT / "MACOS-ARM64-SUBSETS.md").write_text("\n".join(lines) + "\n")
    print(json.dumps({"cargo": {e: {k: v for k, v in d.items() if k.endswith("count") or k in ["missing_text", "unknown_license"]}
                                 for e, d in result["editions"].items()},
                      "npm": {"production": len(production), "build": len(build)}, "locks_unchanged": before == after}))


if __name__ == "__main__":
    main()

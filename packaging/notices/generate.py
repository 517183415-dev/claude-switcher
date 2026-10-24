#!/usr/bin/env python3
"""Offline lock inventory and verbatim local license collection; Python 3.11+."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile
import tomllib


def sha(data):
    return hashlib.sha256(data).hexdigest()


def table_value(value):
    return str(value).replace("|", "\\|").replace("\n", " ")


def index_entry(index, name, version):
    prefix = ("1/" + name if len(name) == 1 else "2/" + name if len(name) == 2
              else "3/" + name[0] + "/" + name if len(name) == 3
              else name[:2] + "/" + name[2:4] + "/" + name)
    file = index / ".cache" / prefix
    if file.is_file():
        for part in file.read_bytes().split(b"\0"):
            if part.startswith(b"{"):
                entry = json.loads(part)
                if entry.get("name") == name and entry.get("vers") == version:
                    return entry
    return None


def collect(directory, explicit=()):
    """Only package license/notice/copyright files; never follow symlinks."""
    if not directory.is_dir() or directory.is_symlink():
        return []
    candidates = set()
    for item in directory.rglob("*"):
        relative = item.relative_to(directory)
        if any(p in ("node_modules", ".git", "target") for p in relative.parts):
            continue
        if item.is_symlink() or not item.is_file():
            continue
        if any((directory / Path(*relative.parts[:i])).is_symlink()
               for i in range(1, len(relative.parts))):
            continue
        if re.match(r"^(licen[cs]e|notice|copyright)([._-]|$)", item.name, re.I) or any(p.lower() in ("licenses", "licences") for p in relative.parts[:-1]):
            candidates.add(item)
    for name in explicit:
        candidate = directory / name
        try:
            relative = candidate.relative_to(directory)
        except ValueError:
            continue
        if ".." not in relative.parts and candidate.is_file() and not candidate.is_symlink():
            candidates.add(candidate)
    return [(str(file.relative_to(directory)), file.read_bytes()) for file in sorted(candidates)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-registry", type=Path, default=Path.home() / ".cargo/registry")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    out = root / "packaging/notices"
    lock_paths = [root / "Cargo.lock", root / "apps/desktop/package-lock.json"]
    initial = {str(p.relative_to(root)): sha(p.read_bytes()) for p in lock_paths}
    cargo = tomllib.loads(lock_paths[0].read_text())["package"]
    npm = json.loads(lock_paths[1].read_text())["packages"]
    registry_name = "index.crates.io-1949cf8c6b5b557f"
    cache = args.cargo_registry / "src" / registry_name
    index = args.cargo_registry / "index" / registry_name
    records = []
    bundles = {"cargo": [], "npm": []}
    supplement_path = out / "supplements/sources.json"
    supplement_manifest = json.loads(supplement_path.read_text()) if supplement_path.is_file() else {"packages": []}
    supplements = {(p["ecosystem"], p["name"], p["version"]): p for p in supplement_manifest["packages"]}

    def add(record, files):
        supplement = supplements.get((record["ecosystem"], record["name"], record["version"]))
        record["upstream_notice_review_required"] = False
        if supplement:
            assert supplement["declared_license"] == record["license"]
            assert supplement.get("locked_checksum", record.get("checksum")) == record.get("checksum")
            assert supplement.get("locked_integrity", record.get("integrity")) == record.get("integrity")
            supplemental_files = []
            for evidence in supplement["files"]:
                relative = Path(evidence["file"])
                assert not relative.is_absolute() and ".." not in relative.parts
                data = (out / "supplements" / relative).read_bytes()
                assert sha(data) == evidence["sha256"], f"Supplement changed: {relative}"
                if "not license terms" not in evidence["role"] and "evidence;" not in evidence["role"] and "public fields only" not in evidence["role"]:
                    supplemental_files.append(("supplements/" + str(relative), data))
            files = [*files, *supplemental_files]
            record["supplemental_evidence"] = supplement
            record["upstream_notice_review_required"] = supplement["review_required"]
            record["supplement_text_integrity"] = "vendored SHA256 verified; upstream version/commit or parent applicability recorded in supplements/sources.json"
        record["texts"] = []
        record["text_status"] = "canonical terms plus upstream declaration; project copyright notice absent" if record["upstream_notice_review_required"] else "collected" if files else "local package unavailable" if not record["local_source"] else "no matching license/notice file in local package"
        for name, data in files:
            # Preserve the original text verbatim. A hash detects any decoding loss.
            text = data.decode("utf-8")
            record["texts"].append({"path": name, "sha256": sha(data), "bytes": len(data)})
            bundles[record["ecosystem"]].append(
                f"\n===== {record['name']}@{record['version']} :: {name} :: SHA256 {sha(data)} =====\n"
                + text + "\n")
        records.append(record)

    for package in cargo:
        if "source" not in package:
            continue
        if package["source"] != "registry+https://github.com/rust-lang/crates.io-index":
            raise RuntimeError("Unsupported Cargo source; explicitly qualify its local cache first")
        name, version = package["name"], package["version"]
        directory = cache / f"{name}-{version}"
        manifest = directory / "Cargo.toml"
        metadata = tomllib.loads(manifest.read_text())["package"] if manifest.is_file() else {}
        archive = args.cargo_registry / "cache" / registry_name / f"{name}-{version}.crate"
        archive_files = None
        if archive.is_file():
            if sha(archive.read_bytes()) != package["checksum"]:
                raise RuntimeError(f"Cached crate archive differs from lock: {name}@{version}")
            with tarfile.open(archive, "r:gz") as tar:
                prefix = f"{name}-{version}/"
                member = tar.getmember(prefix + "Cargo.toml")
                metadata = tomllib.loads(tar.extractfile(member).read().decode("utf-8"))["package"]
                archive_files = []
                for member in tar.getmembers():
                    if not member.isfile() or not member.name.startswith(prefix):
                        continue
                    relative = Path(member.name[len(prefix):])
                    if ".." in relative.parts or relative.is_absolute():
                        raise RuntimeError(f"Invalid crate archive path: {name}@{version}")
                    if re.match(r"^(licen[cs]e|notice|copyright)([._-]|$)", relative.name, re.I) or any(p.lower() in ("licenses", "licences") for p in relative.parts[:-1]) or str(relative) == metadata.get("license-file"):
                        archive_files.append((str(relative), tar.extractfile(member).read()))
                archive_files.sort()
        entry = index_entry(index, name, version)
        if entry and entry.get("cksum") != package["checksum"]:
            raise RuntimeError(f"Registry index checksum differs from lock: {name}@{version}")
        license_id = metadata.get("license") or (entry or {}).get("license") or "UNKNOWN"
        explicit = [metadata["license-file"]] if metadata.get("license-file") else []
        files = archive_files if archive_files is not None else collect(directory, explicit)
        checksum_file = directory / ".cargo-checksum.json"
        checksums = json.loads(checksum_file.read_text()) if checksum_file.is_file() else None
        if files and archive_files is None and checksums:
            if checksums.get("package") != package["checksum"]:
                raise RuntimeError(f"Cache package checksum differs from lock: {name}@{version}")
            for path, data in files:
                if checksums["files"].get(path) != sha(data):
                    raise RuntimeError(f"Cached license checksum mismatch: {name}@{version}/{path}")
        add({"ecosystem": "cargo", "name": name, "version": version,
             "source": package["source"], "checksum": package["checksum"],
             "license": license_id,
             "license_evidence": "Cargo.toml" if metadata.get("license") else "cached registry index" if entry and entry.get("license") else "missing",
             "scope": "overinclusive: targets/features/build/proc-macro/runtime unresolved",
             "text_integrity": "crate archive SHA256 matches lock" if archive_files is not None else "cargo checksum metadata" if checksums else "source cache only; not authenticated against archive",
             "local_source": bool(metadata), "license_file_declared": metadata.get("license-file")}, files)

    for path, package in sorted(npm.items()):
        if not path:
            continue
        name = path.rsplit("node_modules/", 1)[-1]
        directory = root / "apps/desktop" / path
        manifest = directory / "package.json"
        metadata = json.loads(manifest.read_text()) if manifest.is_file() else {}
        if metadata and (metadata.get("name") != name or metadata.get("version") != package["version"]):
            raise RuntimeError(f"Installed npm version/name differs from lock: {path}")
        add({"ecosystem": "npm", "name": name, "version": package["version"],
             "source": package.get("resolved", "UNKNOWN"), "integrity": package.get("integrity"),
             "license": package.get("license") or metadata.get("license") or "UNKNOWN",
             "license_evidence": "package-lock.json" if package.get("license") else "installed package.json" if metadata.get("license") else "missing",
             "scope": "dev/build graph (npm dev flag)" if package.get("dev") else "production dependency graph; bundled subset unresolved",
             "optional": bool(package.get("optional")), "lock_path": path,
             "local_source": bool(metadata)}, collect(directory))

    final = {str(p.relative_to(root)): sha(p.read_bytes()) for p in lock_paths}
    if initial != final:
        raise RuntimeError("Lockfile changed while collecting notices; no output written")
    summary = {}
    for ecosystem in bundles:
        group = [r for r in records if r["ecosystem"] == ecosystem]
        summary[ecosystem] = {"packages": len(group), "texts_present": sum(bool(r["texts"]) for r in group),
                              "texts_missing": sum(not r["texts"] for r in group),
                              "upstream_notice_review_required": sum(r["upstream_notice_review_required"] for r in group),
                              "license_unknown": sum(r["license"] == "UNKNOWN" for r in group)}
        (out / f"{ecosystem}-license-texts.txt").write_text("".join(bundles[ecosystem]))
    inventory = {"schema_version": 1, "mode": "offline-overinclusive-lock-inventory",
                 "lock_sha256_before": initial, "lock_sha256_after": final,
                 "summary": summary, "packages": records}
    (out / "inventory.json").write_text(json.dumps(inventory, indent=2, ensure_ascii=False) + "\n")
    missing = ["# 许可材料缺项与上游通知待核\n", "第一表逐项列出本次没有收集到许可材料的锁定包。第二表保留使用规范条款后仍未关闭的上游项目专属通知缺口。",
               "缺项不表示包没有许可证。范围包括其他平台、可选项与构建依赖。禁止将此表当作发行许可放行。\n",
               "| 生态 | 包 | 版本 | 许可证标识 | 缺项原因 |", "|---|---|---|---|---|"]
    missing += [f"| {r['ecosystem']} | {r['name']} | {r['version']} | {table_value(r['license'])} | {r['text_status']} |"
                for r in records if not r["texts"]]
    missing += ["\n## 已有标准条款，但上游项目专属通知仍待核\n",
                "标准条款包含原始版权占位符。不得凭作者姓名填写占位符，也不得把下表视为通知完整。\n",
                "| 生态 | 包 | 版本 | 未关闭项 |", "|---|---|---|---|"]
    missing += [f"| {r['ecosystem']} | {r['name']} | {r['version']} | {r['supplemental_evidence']['notice_status']} |"
                for r in records if r["upstream_notice_review_required"]]
    (out / "MISSING.md").write_text("\n".join(missing) + "\n")
    lines = ["# 第三方依赖与许可声明材料\n",
             "本材料绑定当前锁文件。它是离线生成的 **overinclusive 锁全集**，不是最终二进制 SBOM，也不是法律审查通过证明。",
             "生成器只读取项目锁文件、项目 node_modules、本机 crates.io registry 缓存和已归档的 supplements。生成器不联网、不安装或升级依赖。补充材料曾从官方发布源与精确提交采集。\n",
             "## 范围与打包边界\n",
             "1. Cargo 表包含根 Cargo.lock 的全部第三方版本。排除四个第一方 workspace 包，以及 experiments 的独立锁文件。",
             "2. Cargo.lock 不区分运行时、proc-macro、构建工具、平台与 feature。此表保留所有目标条目，不能断言它们全部进入 macOS 工件。",
             "3. npm 表保留 package-lock.json 的全部条目。dev 标记说明构建依赖图；其他条目属于 production 依赖图。Vite/Svelte 会编译、内联和消除代码，锁文件不能证明最终保留范围。",
             "4. 未分析最终 app/helper 的链接映射与前端 bundle。不覆盖系统 SDK、编译器、系统动态库、Claude 应用及其账号数据。",
             "5. 发行负责人仍须确认目标、features、实际工件、许可选择及再分发义务，并补齐适用包的缺失正文。\n",
             "## 收集方法与限制\n",
             "Cargo license 标识优先取精确版本的缓存 Cargo.toml，再取同版本本地 sparse registry index。index checksum 必须匹配 Cargo.lock。",
             "Cargo 正文优先取本地 .crate 原始包，完整归档 SHA256 必须等于 Cargo.lock checksum。否则取源码缓存，有 .cargo-checksum.json 时校验；无校验材料时单独标注。npm 标识优先取锁文件；本地 package.json 必须同名同版本。",
             "npm 正文来自已安装目录，未对 npm tarball integrity 作独立复算。清单记录原始 integrity、文本文件路径和 SHA256。",
             "递归收集名称为 LICENSE/LICENCE/NOTICE/COPYRIGHT 的文件，以及 Cargo license-file 指定文件；跳过 symlink、嵌套 node_modules、.git 与 target。",
             "这是按文件名采集，不能保证捕获仅存在源码头注释中的声明或任意命名的许可证。双重许可表达式照录，未代替作者选择分支。\n",
             "补充材料逐包记录精确 .crate VCS 提交或 npm 发布 gitHead，以及来源 URL、适用依据和文本摘要。同版本 CLI/rolldown 主包全文用于其精确版本平台绑定，并保留主包 tarball 对锁文件 SRI 校验依据。",
             "objc2 系列和两个 Rich Harris 包没有上游项目专属完整版权通知。材料保留精确提交的原始许可声明，并附官方标准许可条款，保留版权占位符。**条款已归档不代表这 13 项通知缺口关闭。**",
             "这些包在 inventory 的 upstream_notice_review_required 中单列。不得用规范模板代替真实版权署名，不得把此统计作为法律审定。详见 [补充证据说明](supplements/README.md)。\n",
             "## 本次覆盖\n", "| 生态 | 第三方包条目 | 已归档许可材料 | 材料缺项 | 上游专属通知待核 | 标识 UNKNOWN |", "|---|---:|---:|---:|---:|---:|"]
    for name, count in summary.items():
        lines.append(f"| {name} | {count['packages']} | {count['texts_present']} | {count['texts_missing']} | {count['upstream_notice_review_required']} | {count['license_unknown']} |")
    lines += ["\n正文缺项逐项见 [MISSING.md](notices/MISSING.md)。所有条目的文件归属、摘要和证据来源见 [inventory.json](notices/inventory.json)。",
              "完整原文：[Cargo](notices/cargo-license-texts.txt)、[npm](notices/npm-license-texts.txt)。分隔行提供包名、版本和原始文件名，正文保持 UTF-8 原文。\n",
              "## 锁文件不变性\n", "生成前后 SHA256 一致：\n", "| 文件 | SHA256 |", "|---|---|"]
    lines += [f"| `{path}` | `{digest}` |" for path, digest in initial.items()]
    lines += ["\n## 复现\n", "在项目根目录执行 `python3 packaging/notices/generate.py`。需要 Python 3.11+ 和相同锁文件、本地依赖缓存。",
              "可用 `--cargo-registry /path/to/.cargo/registry` 明确指定缓存。脚本不联网。缓存覆盖变化会改变正文覆盖计数。\n"]
    for ecosystem in bundles:
        lines += [f"## {ecosystem} 锁定清单\n", "| 包 | 版本 | 来源 | license 标识 | 图边界 | 正文文件数 |", "|---|---|---|---|---|---:|"]
        for record in records:
            if record["ecosystem"] == ecosystem:
                lines.append("| " + " | ".join(table_value(record[key]) for key in ("name", "version", "source", "license", "scope")) + f" | {len(record['texts'])} |")
        lines.append("")
    (root / "packaging/THIRD-PARTY-NOTICES.md").write_text("\n".join(lines) + "\n")
    print(json.dumps({"locks_unchanged": initial == final, "summary": summary}, ensure_ascii=False))


if __name__ == "__main__":
    main()

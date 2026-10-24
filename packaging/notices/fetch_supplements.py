#!/usr/bin/env python3
"""Explicit network collection from locked archives and official upstream commits.

Normal generation remains offline. This optional evidence refresh uses only
the fixed package list below and writes inside notices/supplements.
"""
import base64
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "packaging/notices/supplements"
CRATES = ["alloc-stdlib@0.3.0", "block2@0.6.2", "dispatch2@0.3.1", "objc2@0.6.5",
          "objc2-app-kit@0.3.2", "objc2-core-foundation@0.3.2", "objc2-encode@4.1.0",
          "objc2-exception-helper@0.1.1", "objc2-foundation@0.3.2", "objc2-osa-kit@0.3.2",
          "objc2-quartz-core@0.3.2", "objc2-web-kit@0.3.2", "selectors@0.38.0",
          "tauri-plugin@2.7.2", "ts-rs@12.0.1", "ts-rs-macros@12.0.1"]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "claude-switcher-license-evidence"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def main():
    OUT.mkdir(exist_ok=True)
    cargo_lock = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    lock = {(p["name"], p["version"]): p for p in cargo_lock}
    npm = json.loads((ROOT / "apps/desktop/package-lock.json").read_text())["packages"]
    records = []
    cache = Path.home() / ".cargo/registry/cache/index.crates.io-1949cf8c6b5b557f"
    standard_cache = {}

    def save(folder, name, content, url, role):
        target = OUT / folder / name
        target.parent.mkdir(exist_ok=True, parents=True)
        target.write_bytes(content)
        return {"file": str(target.relative_to(OUT)), "sha256": sha(content),
                "source_url": url, "role": role}

    def remote(folder, name, url, role):
        return save(folder, name, fetch(url), url, role)

    def canonical(folder, license_id):
        # The standard license contains placeholders where upstream omitted its
        # project-specific copyright notice. Never replace them with guesses.
        if license_id not in standard_cache:
            url = f"https://raw.githubusercontent.com/spdx/license-list-data/v3.28.0/text/{license_id}.txt"
            standard_cache[license_id] = (url, fetch(url))
        url, content = standard_cache[license_id]
        return save(folder, f"CANONICAL-{license_id}.txt", content, url,
                    "canonical terms; preserves template placeholders; not an upstream copyright notice")

    for spec in CRATES:
        name, version = spec.split("@")
        archive = cache / f"{name}-{version}.crate"
        raw = archive.read_bytes()
        assert sha(raw) == lock[name, version]["checksum"]
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as tar:
            prefix = f"{name}-{version}/"
            manifest_bytes = tar.extractfile(prefix + "Cargo.toml").read()
            package = tomllib.loads(manifest_bytes.decode())["package"]
            vcs_bytes = tar.extractfile(prefix + ".cargo_vcs_info.json").read()
            vcs = json.loads(vcs_bytes)
            assert package["name"] == name and package["version"] == version
            repository, commit = package["repository"], vcs["git"]["sha1"]
            assert repository.startswith("https://github.com/")
            repo = repository.removeprefix("https://github.com/")
            raw_base = f"https://raw.githubusercontent.com/{repo}/{commit}"
            folder = "cargo-" + name + "-" + version
            record = {"ecosystem": "cargo", "name": name, "version": version,
                      "locked_checksum": lock[name, version]["checksum"], "declared_license": package["license"],
                      "repository": repository, "commit": commit, "path_in_vcs": vcs["path_in_vcs"],
                      "vcs_info_sha256": sha(vcs_bytes), "normalized_manifest_sha256": sha(manifest_bytes),
                      "binding": "commit/path from .cargo_vcs_info.json inside SHA256-verified locked .crate",
                      "files": []}
            if repo == "madsmtm/objc2":
                record["files"].append(remote(folder, "UPSTREAM-LICENSE.md", raw_base + "/LICENSE.md", "exact-commit upstream licensing declaration and Apple SDK caveat"))
                for license_id in [x.strip() for x in package["license"].split(" OR ")]:
                    record["files"].append(canonical(folder, license_id))
                record["notice_status"] = "upstream provides license declaration/links, but no project-specific copyright notice; canonical terms keep placeholders"
                record["review_required"] = True
            elif name == "selectors":
                # Its own source header expressly incorporates this canonical MPL.
                source = tar.extractfile(prefix + "lib.rs").read().decode()
                header = source.split("*/", 1)[0] + "*/\n"
                assert "https://mozilla.org/MPL/2.0/" in header
                record["files"].append(save(folder, "UPSTREAM-SOURCE-LICENSE-NOTICE.txt", header.encode(), raw_base + "/selectors/lib.rs", "verbatim package source license header"))
                record["files"].append(remote(folder, "MPL-2.0.txt", "https://www.mozilla.org/MPL/2.0/index.txt", "full license expressly referenced by exact locked source header"))
                record["notice_status"] = "locked source notice plus officially referenced complete MPL-2.0 terms"
                record["review_required"] = False
            else:
                names = ["LICENSE-APACHE-2.0", "LICENSE-MIT"] if name == "tauri-plugin" else ["LICENSE"]
                for filename in names:
                    record["files"].append(remote(folder, filename, raw_base + "/" + filename, "exact-commit repository-wide license for published member crate"))
                record["notice_status"] = "exact-commit upstream full project license"
                record["review_required"] = False
            # The upstream member manifest establishes path applicability; the
            # published normalized manifest and verified vcs-info bind version.
            record["files"].append(remote(folder, "UPSTREAM-MEMBER-Cargo.toml", raw_base + "/" + vcs["path_in_vcs"] + "/Cargo.toml", "applicability evidence; not license terms"))
            records.append(record)

    for name, version, repo in [("is-reference", "3.0.3", "Rich-Harris/is-reference"),
                                ("locate-character", "3.0.0", "Rich-Harris/locate-character")]:
        key = "node_modules/" + name
        url = "https://registry.npmjs.org/" + name + "/" + version
        metadata = json.loads(fetch(url))
        assert metadata["name"] == name and metadata["version"] == version
        assert metadata["dist"]["integrity"] == npm[key]["integrity"]
        commit = metadata["gitHead"]
        base = f"https://raw.githubusercontent.com/{repo}/{commit}"
        folder = "npm-" + name + "-" + version
        upstream_manifest = fetch(base + "/package.json")
        manifest = json.loads(upstream_manifest)
        assert manifest["name"] == name and manifest["version"] == version and manifest["license"] == "MIT"
        readme = fetch(base + "/README.md").decode()
        # These exact commits have no LICENSE file. Preserve their actual MIT
        # declaration rather than inventing an attribution or borrowing one.
        license_section = readme[readme.rindex("## License"):]
        tree_url = f"https://api.github.com/repos/{repo}/git/trees/{commit}?recursive=1"
        tree = json.loads(fetch(tree_url))
        license_paths = [p["path"] for p in tree["tree"] if "license" in p["path"].lower() or "copying" in p["path"].lower()]
        assert not license_paths
        files = [save(folder, "UPSTREAM-README-LICENSE-SECTION.md", license_section.encode(), base + "/README.md", "verbatim exact-commit package license declaration"),
                 save(folder, "UPSTREAM-package.json", upstream_manifest, base + "/package.json", "applicability evidence; not license terms"),
                 canonical(folder, "MIT")]
        safe_metadata = {"name": name, "version": version, "license": metadata["license"],
                         "gitHead": commit, "repository": metadata["repository"],
                         "dist_integrity": metadata["dist"]["integrity"], "tree_license_paths": license_paths,
                         "tree_url": tree_url}
        files.append(save(folder, "REGISTRY-BINDING.json", (json.dumps(safe_metadata, indent=2) + "\n").encode(), url, "registry version/gitHead binding; public fields only"))
        records.append({"ecosystem": "npm", "name": name, "version": version,
                        "locked_integrity": npm[key]["integrity"], "declared_license": "MIT",
                        "commit": commit, "repository": "https://github.com/" + repo,
                        "binding": "official registry exact-version gitHead and integrity; same-commit manifest name/version/license verified",
                        "notice_status": "upstream exact commit has only MIT declaration, no full project license/copyright notice; canonical terms keep placeholders",
                        "review_required": True, "files": files})

    for name, parent, version in [("@rolldown/binding-darwin-arm64", "rolldown", "1.2.15"),
                                   ("@tauri-apps/cli-darwin-arm64", "@tauri-apps/cli", "2.12.4")]:
        entry = npm["node_modules/" + parent]
        tarball = fetch(entry["resolved"])
        algorithm, expected = entry["integrity"].split("-", 1)
        assert base64.b64encode(hashlib.new(algorithm, tarball).digest()).decode() == expected
        folder = "npm-" + name.replace("@", "").replace("/", "-") + "-" + version
        files = []
        with tarfile.open(fileobj=io.BytesIO(tarball), mode="r:gz") as tar:
            manifest_bytes = tar.extractfile("package/package.json").read()
            manifest = json.loads(manifest_bytes)
            assert manifest["name"] == parent and manifest["version"] == version
            assert manifest["optionalDependencies"][name] == version
            for member in tar.getmembers():
                relative = member.name.removeprefix("package/")
                if member.isfile() and "/" not in relative and relative.upper().startswith(("LICENSE", "THIRD-PARTY-LICENSE", "NOTICE")):
                    files.append(save(folder, relative, tar.extractfile(member).read(), entry["resolved"], "same-version full parent-package license; parent ships exact-version platform binding"))
        target_meta = json.loads(fetch("https://registry.npmjs.org/" + name.replace("/", "%2f") + "/" + version))
        assert target_meta["license"] == manifest["license"]
        assert target_meta["dist"]["integrity"] == npm["node_modules/" + name]["integrity"]
        public_evidence = {"name": parent, "version": version, "license": manifest["license"],
                           "repository": manifest["repository"], "platform_binding": {name: manifest["optionalDependencies"][name]},
                           "parent_lock_integrity": entry["integrity"], "parent_tarball_sha256": sha(tarball),
                           "platform_license": target_meta["license"], "platform_repository": target_meta["repository"],
                           "platform_lock_integrity": target_meta["dist"]["integrity"]}
        files.append(save(folder, "PARENT-APPLICABILITY.json", (json.dumps(public_evidence, indent=2) + "\n").encode(), entry["resolved"], "version/license/repository/optional binding evidence; parent tarball SRI verified"))
        records.append({"ecosystem": "npm", "name": name, "version": version,
                        "locked_integrity": npm["node_modules/" + name]["integrity"], "declared_license": manifest["license"],
                        "binding": "same-version parent published tarball integrity verified against lock; exact optional binding plus same upstream license/repository",
                        "notice_status": "full parent-package notices apply to same-version platform binding; additional binary-linked third parties not independently inventoried",
                        "review_required": False, "files": files})
    manifest = {"schema_version": 1, "purpose": "fixed-version supplemental license evidence; not legal review",
                "packages": sorted(records, key=lambda p: (p["ecosystem"], p["name"], p["version"]))}
    (OUT / "sources.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    print(f"Collected official supplementary evidence for {len(records)} locked packages")


if __name__ == "__main__":
    main()

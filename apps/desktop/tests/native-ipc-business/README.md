# Controlled business IPC regression

These Python standard-library scripts build a feature-isolated fixture. They register the real activity preview/export commands with synthetic state. They never start the ordinary application. macOS, installed Rust, an existing offline Cargo cache and `sandbox-exec` are prerequisites. No dependencies are installed.

Always use a dedicated owner-only output directory. Every run uses a new directory. A build attempt refuses an existing destination. Do not use historical evidence as the new output root.

```sh
python3 -B apps/desktop/tests/native-ipc-business/test_runner.py
python3 -B apps/desktop/tests/native-ipc-business/runner.py prepare --output /absolute/private/output
python3 -B apps/desktop/tests/native-ipc-business/build_native.py personal --attempt r1 --output /absolute/private/output --toolchain-bin /existing/.rustup/toolchains/VERSION/bin --cargo-home /existing/offline/cache --target-dir /absolute/private/output/target
python3 -B apps/desktop/tests/native-ipc-business/runner.py run --output /absolute/private/output --edition personal --mode preflight --build-attempt r1
python3 -B apps/desktop/tests/native-ipc-business/runner.py run --output /absolute/private/output --edition personal --mode business --build-attempt r1
```

Repeat the build and both modes for `public`, sequentially. `--check` validates build configuration without compiling. Keep source frozen until all builds and runs finish. A shared target directory requires exclusive sequential ownership.

`support.py` derives the project root from its own location. It binds all relevant Rust/config/fixture/script inputs, excluding documented generated schema/permission outputs. Receipts contain the complete before/after map, harness hashes, signed binary hash, generated sandbox hash and command/log hashes. Runtime recomputes the complete map, including added files, before and after launch. It rejects a changed receipt/binary/source combination. Old receipts remain historical evidence after script changes; rebuild to establish a current binding.

Each process receives a whitelist environment and a new synthetic HOME/TMPDIR. Runtime denies every network operation including localhost. Before any fixture launch, negative controls must prove private-directory metadata open, forbidden execution and public/localhost network operations are denied. The metadata probe immediately closes any unexpected descriptor; it never reads or lists real HOME. Profiles are generated from current explicit paths and retained with each run. No automatic policy relaxation exists.

The independent parser checks denial classifications, fixed export paths, redaction, raw historical positive control, file contents and 0700/0600 permissions. Success proves this controlled real-command WKWebView path. It does not prove ordinary startup, real accounts, Keychain, installation or system-service behavior. Timeout termination covers only the fixture process group; separate WebKit processes are not claimed terminated.

Build paths reject symlink ancestors. The target must be the project `target` or below the explicit output `target`. The toolchain must be a dedicated `.rustup/toolchains/VERSION/bin`. Cargo HOME must be an isolated cache. Only exact registry/git dependency subtree links may resolve to the account Cargo cache; credentials and config files stay denied. Synthetic credentials/config controls execute before compilation. Real HOME comes from the OS account record, independently of the calling environment HOME. Source manifests, directories and files reject links. Evidence directories require owner-only 0700; receipts and binaries reject symlinks, hardlinks and writable sharing.

For cross-process persistence, first build both editions into one fresh output root using the commands above. Then run:

```sh
python3 -B apps/desktop/tests/native-ipc-business/test_persistence.py
python3 -B apps/desktop/tests/native-ipc-business/persistence.py --output /absolute/private/output --build-attempt r1
```

This creates one new synthetic session and launches four separate fixture processes, waiting for each exit: Personal write, Public write, Personal read, Public read. The two editions persist different nondefault settings. Readers call the real `DesktopState.preferences` method through the existing feature-only info command. Writers invoke the real `save_preferences` command. Each phase preserves the other edition's file bytes. Readers preserve all persistent bytes. This session reuses only its own synthetic HOME/TMPDIR/root. Its unchanged sandbox denies all network and real private locations. Named process logs are created exclusively and cannot overwrite prior evidence.

Encrypted backup uses `runner.py run --mode backup` after both edition builds. Personal exports a synthetic backup through a feature-only command into WebView memory. Five IPC restore calls exercise wrong password, tampering, truncation, MemoryVault write rejection, then success. Each call uses the production shared restore body and `Publisher.restore_backup`, closes the publisher and reopens it. Native receipts contain only random generation IDs and boolean/count invariants. They contain no passphrases, keys, credentials, state snapshots or encrypted bytes. Python independently validates the native/WebView receipt agreement, final active generation, old/current public draft flags and all 0700/0600 permissions. Public must return permission denial for both feature-only backup commands and leave its synthetic root empty. The Personal-only handler list statically excludes both commands from Public. Runtime permission rejection occurs before handler lookup and does not independently prove an absent handler. No production Keychain handler or real Keychain is exercised. Run `test_backup.py` for the independent parser regressions.

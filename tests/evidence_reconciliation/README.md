# Rust evidence reconciliation

Run `python3 -B -m unittest tests.evidence_reconciliation.test_reconcile -v` without Cargo or network access.

The parser accepts only exact libtest statuses. `ok` contributes to `top_level_passed_records`. `ignored` and `ignored, <reason>` contribute only to `top_level_ignored_records`; `ignored_tests` records each harness, name and verbatim reason (`null` when no reason is printed). `top_level_total_records` includes both statuses. Distinct passed names and distinct total names are reported separately. Failed or unknown statuses remain errors.

Each harness must reconcile its declared total, passed count and ignored count with individual records. The complete passed-plus-ignored name multiset must match that same harness in the independent `--list` log. Nested F-06 child fixtures must still pass; they cannot be ignored. An accounting result never turns an ignored test into a pass. Receipt reconciliation still rejects failed lint commands, changed source snapshots or log hash mismatches.

Generated logs and replay receipts belong under the dedicated `output/` evidence directory, not this source tree.

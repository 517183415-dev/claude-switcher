# Independent actual Rust YAML verification

Input: `f10-fixtures.json`, 44 actual Rust receipts: 13 accept, 31 reject.
No expected output was generated. No dependencies were installed.

- PyYAML 6.0.3: 42 PASS, 2 FAIL, 0 BLOCKED; exit 1.
- All 31 rejection receipts contain no output.
- Eleven accepted outputs pass complete document, root mapping, unique top-level key, 13 managed rules, exact user bytes, nested semantic preservation, and actual second-pass idempotence checks.
- `tabs` and `crlf-comment`: PyYAML rejects the preserved header Tab in both original and actual output. This parser failure remains in the report.
- Existing Ruby 2.6.10 / Psych 3.1.0 parses those two original inputs and their exact Rust outputs: 4/4 parse PASS. Both cases also pass 13-rule, ordering, exact user bytes, nested semantic, changed, and actual second-pass checks; exit 0.
- This establishes a parser compatibility difference. Actual Clash runtime validation was NOT_RUN.

## Initial validator corrections

`yaml-parser-initial-report.json` preserves the initial 36 PASS / 8 FAIL result.
Six failures belonged to two validator assumptions, not actual Rust data loss:

1. Five new-section cases include one generated blank separator after the managed section. The validator now excludes exactly that generated separator only when removing the newly inserted root header. It still compares every original user byte exactly, allowing only a required final newline where absent.
2. Legacy migration intentionally removes an old managed section containing a duplicate root key. The validator now strips that managed section before parsing the original user document. It continues to require a unique root key in actual output.

The two PyYAML Tab failures were not suppressed or normalized. `yaml-parser-psych-report.json` retains exact original/output text and SHA256 values. Command receipts contain real argv and exit codes.

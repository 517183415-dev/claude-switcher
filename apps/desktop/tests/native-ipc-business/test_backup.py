"""Independent receipt acceptance and non-disclosure regression tests; no native launch."""
import copy
import json
import unittest
import backup


class BackupReceiptTests(unittest.TestCase):
    def rows(self):
        result = []
        for case in backup.CASES:
            success = case == 'success'
            result.append(dict(case=case, entered=1, accepted=success,
                               error_code={'wrong_password': 'password_or_integrity', 'tampered': 'password_or_integrity',
                                           'truncated': 'header', 'vault_failure': 'vault_unavailable', 'success': None}[case],
                               old_generation='a' * 32, new_generation=('b' if success else 'a') * 32,
                               active_unchanged=not success, old_generation_preserved=True,
                               trust_unchanged=True, old_vault_preserved=True, vault_items=2 if success else 1,
                               reopened_credentials_match=True, reopened_slots=[True, False, True] if success else [False] * 3,
                               recovery_required=success, source_unchanged=True, old_assets_preserved=True, source_assets_unchanged=True,
                               reopened_asset_bytes_match=True, reopened_asset_reference_matches=True, reopened_asset_validated=True))
        return result

    def verify(self, rows=None, events=None):
        rows = self.rows() if rows is None else rows
        return backup.validate_receipts(rows, rows if events is None else events)

    def test_accepts_exact_five_case_crypto_and_reopen_matrix(self):
        self.assertEqual(self.verify()['rejected'], 4)

    def rejected(self, mutate):
        rows = self.rows(); mutate(rows)
        with self.assertRaises(AssertionError): self.verify(rows)

    def test_rejects_wrong_password_reported_as_success(self):
        self.rejected(lambda rows: rows[0].update(accepted=True))

    def test_rejects_unexpected_error_instead_of_vault_failure(self):
        self.rejected(lambda rows: rows[3].update(error_code='unexpected_restore_error'))

    def test_rejects_failure_switching_active_generation(self):
        self.rejected(lambda rows: rows[1].update(new_generation='c' * 32))

    def test_rejects_old_generation_damage(self):
        self.rejected(lambda rows: rows[2].update(old_generation_preserved=False))

    def test_rejects_changed_old_vault(self):
        self.rejected(lambda rows: rows[3].update(old_vault_preserved=False))

    def test_rejects_missing_reopened_credentials(self):
        self.rejected(lambda rows: rows[4].update(reopened_credentials_match=False))

    def test_rejects_default_draft_instead_of_restored_backup(self):
        self.rejected(lambda rows: rows[4].update(reopened_slots=[False] * 3))

    def test_rejects_missing_recovery_gate_after_success(self):
        self.rejected(lambda rows: rows[4].update(recovery_required=False))

    def test_rejects_source_mutation(self):
        self.rejected(lambda rows: rows[0].update(source_unchanged=False))

    def test_rejects_secret_or_ciphertext_fields_in_receipt(self):
        self.rejected(lambda rows: rows[0].update(bytes=[1, 2, 3]))

    def test_rejects_boolean_as_vault_count(self):
        self.rejected(lambda rows: rows[0].update(vault_items=True))

    def test_rejects_different_native_and_webview_receipts(self):
        events = copy.deepcopy(self.rows()); events[0]['entered'] = 0
        with self.assertRaises(AssertionError): self.verify(events=events)

    def test_rejects_old_asset_damage(self): self.rejected(lambda rows: rows[0].update(old_assets_preserved=False))
    def test_rejects_source_asset_damage(self): self.rejected(lambda rows: rows[1].update(source_assets_unchanged=False))
    def test_rejects_wrong_reopened_asset_bytes(self): self.rejected(lambda rows: rows[4].update(reopened_asset_bytes_match=False))
    def test_rejects_missing_reopened_asset_reference(self): self.rejected(lambda rows: rows[4].update(reopened_asset_reference_matches=False))
    def test_rejects_unvalidated_reopened_asset(self): self.rejected(lambda rows: rows[4].update(reopened_asset_validated=False))

    def test_rejects_missing_native_receipt(self):
        with self.assertRaises(AssertionError): self.verify(events=self.rows()[:-1])

    def test_rejects_case_reordering(self):
        self.rejected(lambda rows: rows.reverse())

    def test_public_requires_both_commands_to_be_permission_denied(self):
        backup.validate_public([{'command': name, 'kind': 'permission'} for name in backup.COMMANDS])

    def test_public_rejects_arbitrary_failure_as_acl_denial(self):
        with self.assertRaises(AssertionError):
            backup.validate_public([{'command': name, 'kind': 'other'} for name in backup.COMMANDS])

    def test_public_rejects_missing_instead_of_exact_runtime_permission(self):
        with self.assertRaises(AssertionError):
            backup.validate_public([{'command': name, 'kind': 'missing'} for name in backup.COMMANDS])

if __name__ == '__main__': unittest.main()

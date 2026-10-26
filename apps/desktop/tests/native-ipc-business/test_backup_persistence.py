import base64
import copy
import hashlib
import json
import unittest

import backup_persistence


class ReadOracleTests(unittest.TestCase):
    def setUp(self):
        image = b'synthetic image bytes'
        digest = hashlib.sha256(image).hexdigest()
        generation = 'a' * 32
        draft = {'slots': [{'campaigns': [{'image': {'sha256': digest}}]}]}
        base = 'v2/personal/publisher/'
        trust = b'{"key":"synthetic"}'
        self.snapshot = {'directories': [], 'files': {
            base + generation + '/state.json': json.dumps({'draft': draft}).encode(),
            base + generation + '/assets/' + digest: image,
            base + 'trust.json': trust,
            'v2/personal/backup-fixture/source/trust.json': trust,
        }}
        self.response = {'draft': draft, 'preview': 'data:image/png;base64,' + base64.b64encode(image).decode()}
        self.proof = {'new_generation': generation, 'asset_sha256': digest}

    def test_matching_independent_disk_and_production_reads(self):
        backup_persistence.validate_read(self.response, self.snapshot, copy.deepcopy(self.snapshot), self.proof)

    def test_changed_disk_draft_image_preview_and_trust_rejected(self):
        for mutation in ('disk', 'draft', 'preview', 'trust'):
            with self.subTest(mutation=mutation):
                before = copy.deepcopy(self.snapshot)
                after = copy.deepcopy(self.snapshot)
                response = copy.deepcopy(self.response)
                if mutation == 'disk':
                    after['files']['extra'] = b'changed'
                elif mutation == 'draft':
                    response['draft']['slots'] = []
                elif mutation == 'preview':
                    response['preview'] = 'data:image/png;base64,' + base64.b64encode(b'wrong').decode()
                else:
                    after['files']['v2/personal/publisher/trust.json'] = b'{"key":"wrong"}'
                    before = copy.deepcopy(after)
                with self.assertRaises(AssertionError):
                    backup_persistence.validate_read(response, before, after, self.proof)


if __name__ == '__main__':
    unittest.main()

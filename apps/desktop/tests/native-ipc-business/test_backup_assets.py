"""Independent PNG and restored campaign-reference oracle; no App launch."""
import binascii
import copy
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib
import backup_assets


def png(pixel, text=False):
    def chunk(name, value):
        return struct.pack('>I', len(value)) + name + value + struct.pack('>I', binascii.crc32(name + value))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0)) + (chunk(b'tEXt', b'Fixture\0SYNTHETIC_PNG_METADATA') if text else b'') + chunk(b'IDAT', zlib.compress(b'\0' + bytes(pixel))) + chunk(b'IEND', b'')


class AssetOracleTests(unittest.TestCase):
    def test_decodes_synthetic_normalized_pixels(self): self.assertEqual(backup_assets.decode_png(png([17,34,51,255])), bytes([17,34,51,255]))
    def test_rejects_invalid_png_crc(self):
        value = bytearray(png([17,34,51,255])); value[-1] ^= 1
        with self.assertRaises(AssertionError): backup_assets.decode_png(bytes(value))
    def test_rejects_preserved_source_text_metadata(self):
        with self.assertRaises(AssertionError): backup_assets.decode_png(png([17,34,51,255], True))
    def test_rejects_trailing_png_bytes(self):
        with self.assertRaises(AssertionError): backup_assets.decode_png(png([17,34,51,255]) + b'extra')

    def asset_case(self, change=None):
        with tempfile.TemporaryDirectory(prefix='backup-assets-oracle-') as tmp:
            root = Path(tmp); assetdir = root / 'assets'; assetdir.mkdir(mode=0o700)
            data = png([17,34,51,255]); digest = hashlib.sha256(data).hexdigest()
            path = assetdir / digest; path.write_bytes(data); path.chmod(0o600)
            image = dict(sha256=digest, mime='image/png', bytes=len(data), width=1, height=1)
            state = dict(assets=[image], draft=dict(slots=[dict(campaigns=[dict(image=copy.deepcopy(image))]),dict(campaigns=[]),dict(campaigns=[])]))
            if change: change(path, state)
            return backup_assets.validate_generation(root, state, bytes([17,34,51,255]), [])
    def test_accepts_actual_asset_bytes_hash_metadata_and_campaign_reference(self): self.assertEqual(self.asset_case()['width'], 1)
    def test_rejects_changed_asset_bytes(self):
        with self.assertRaises(AssertionError): self.asset_case(lambda p,s: p.write_bytes(b'broken'))
    def test_rejects_wrong_mime(self):
        with self.assertRaises(AssertionError): self.asset_case(lambda p,s: s['assets'][0].update(mime='image/jpeg'))
    def test_rejects_missing_campaign_reference(self):
        with self.assertRaises(AssertionError): self.asset_case(lambda p,s: s['draft']['slots'][0].update(campaigns=[]))
    def test_rejects_campaign_referencing_other_asset(self):
        with self.assertRaises(AssertionError): self.asset_case(lambda p,s: s['draft']['slots'][0]['campaigns'][0]['image'].update(sha256='f'*64))
    def test_rejects_world_readable_asset(self):
        with self.assertRaises(AssertionError): self.asset_case(lambda p,s: p.chmod(0o644))
    def test_rejects_wrong_expected_pixel(self):
        with self.assertRaises(AssertionError): backup_assets.decode_png(png([68,85,102,255]), bytes([17,34,51,255]))

if __name__ == '__main__': unittest.main()

"""Cleanup must select only this run's exact disk image identity."""
from pathlib import Path
import unittest
from run import owned_devices


class OwnedDevicesTests(unittest.TestCase):
    def test_foreign_or_similarly_named_images_cannot_be_selected(self):
        own = Path('/synthetic/run/volume.sparseimage')
        foreign = {'image-path': str(own) + '.other',
                   'system-entities': [{'dev-entry': '/dev/foreign'}]}
        matched = {'image-path': str(own), 'system-entities': [
            {'dev-entry': '/dev/owned'}, {'dev-entry': '/dev/owned-partition'}]}
        self.assertEqual(owned_devices(own, {'images': [foreign]}), [])
        self.assertEqual(owned_devices(own, {'images': [foreign, matched]}),
                         ['/dev/owned', '/dev/owned-partition'])

    def test_ambiguous_image_identity_stops_cleanup(self):
        own = Path('/synthetic/run/volume.sparseimage')
        entry = {'image-path': str(own), 'system-entities': [{'dev-entry': '/dev/a'}]}
        with self.assertRaisesRegex(RuntimeError, 'ambiguous'):
            owned_devices(own, {'images': [entry, entry]})


if __name__ == '__main__':
    unittest.main()

import unittest
from release_brand import asset_prefix

class BrandContract(unittest.TestCase):
    def test_existing_releases_keep_their_names(self):
        self.assertEqual(asset_prefix("0.4.14"), "Fastcloud")
        self.assertEqual(asset_prefix("0.2.8"), "Fastcloud")
    def test_new_releases_use_veirune(self):
        self.assertEqual(asset_prefix("0.4.15"), "Veirune")
        self.assertEqual(asset_prefix("1.0.0"), "Veirune")

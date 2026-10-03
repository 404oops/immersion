"""Release signatures and artifact metadata, using disposable fixtures."""
import base64
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

spec = importlib.util.spec_from_file_location(
    "publish_updates", Path(__file__).resolve().parents[1] / "publish-updates.py"
)
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class PublishUpdatesTests(unittest.TestCase):
    def setUp(self):
        self.key = Ed25519PrivateKey.from_private_bytes(bytes([7]) * 32)
        self.public = self.key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
        self.environment = {
            "IMMERSION_UPDATE_PRIVATE_KEY": base64.b64encode(bytes([7]) * 32).decode(),
            "IMMERSION_UPDATE_PUBLIC_KEY": base64.b64encode(self.public).decode(),
        }

    def test_key_mismatch_fails_before_publication(self):
        self.environment["IMMERSION_UPDATE_PUBLIC_KEY"] = base64.b64encode(bytes(32)).decode()
        with patch.dict(os.environ, self.environment):
            with self.assertRaises(ValueError):
                publisher.signing_key()

    def test_signed_matching_architecture_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            for arch in ("x64", "arm64"):
                (dist / f"ImmersionSetup-0.2.6-windows-{arch}.exe").write_bytes(arch.encode())
            (dist / "Immersion-0.2.6-macos-arm64.zip").write_bytes(b"bundle fixture")
            with patch.dict(os.environ, self.environment), \
                    patch.object(publisher.subprocess, "check_output", return_value=b"signature"), \
                    patch.object(publisher.subprocess, "run") as signer:
                publisher.publish(dist, "0.2.6", Path("sparkle"))
                self.assertEqual(signer.call_count, 1)  # Feed is signed as well as the archive.
            for arch in ("x64", "arm64"):
                path = dist / f"update-windows-{arch}.json"
                self.key.public_key().verify(
                    base64.b64decode(path.with_suffix(".json.sig").read_bytes()), path.read_bytes()
                )
                manifest = json.loads(path.read_bytes())
                self.assertEqual(manifest["target"], f"windows-{arch}")
                self.assertEqual(manifest["size"], len(arch))
                self.assertTrue(manifest["url"].endswith(f"windows-{arch}.exe"))
            item = ET.parse(dist / "appcast-macos-arm64.xml").find("channel/item")
            self.assertEqual(item.find("enclosure").get("length"), "14")
            self.assertTrue(item.find("enclosure").get("url").endswith("macos-arm64.zip"))


if __name__ == "__main__":
    unittest.main()

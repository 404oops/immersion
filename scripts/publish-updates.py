#!/usr/bin/env python3
"""Sign Windows manifests and generate the macOS Sparkle appcast.

Requires cryptography. Private keys are consumed from the environment, never
written to dist or printed. Run on the macOS signing job before publication.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


def signing_key():
    raw = base64.b64decode(os.environ["IMMERSION_UPDATE_PRIVATE_KEY"].strip(), validate=True)
    # Sparkle's current export is a 32-byte seed; accept its legacy seed+public format.
    if len(raw) not in (32, 64):
        raise ValueError("invalid signing seed length")
    key = Ed25519PrivateKey.from_private_bytes(raw[:32])
    public = key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    expected = base64.b64decode(os.environ["IMMERSION_UPDATE_PUBLIC_KEY"], validate=True)
    if public != expected:
        raise ValueError("release signing key does not match embedded public key")
    return key


def windows_manifest(version, target, installer, base):
    content = installer.read_bytes()
    return (json.dumps({
        "version": version, "target": target,
        "url": f"{base}/{installer.name}", "size": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }, separators=(",", ":")) + "\n").encode()


def publish(dist, version, sparkle):
    key = signing_key()
    base = f"https://github.com/404oops/immersion/releases/download/v{version}"
    for arch in ("x64", "arm64"):
        target = f"windows-{arch}"
        installer = dist / f"ImmersionSetup-{version}-{target}.exe"
        body = windows_manifest(version, target, installer, base)
        manifest = dist / f"update-{target}.json"
        manifest.write_bytes(body)
        manifest.with_suffix(".json.sig").write_bytes(base64.b64encode(key.sign(body)))

    archive = dist / f"Immersion-{version}-macos-arm64.zip"
    signature = subprocess.check_output(
        [str(sparkle / "bin/sign_update"), "--ed-key-file", "-", "-p", str(archive)],
        input=os.environ["IMMERSION_UPDATE_PRIVATE_KEY"].encode(),
    ).decode().strip()
    ns = "http://www.andymatuschak.org/xml-namespaces/sparkle"
    ET.register_namespace("sparkle", ns)
    rss = ET.Element("rss", {"version": "2.0"})
    channel = ET.SubElement(rss, "channel")
    ET.SubElement(channel, "title").text = "Immersion"
    item = ET.SubElement(channel, "item")
    ET.SubElement(item, "title").text = f"Immersion {version}"
    ET.SubElement(item, f"{{{ns}}}version").text = version
    ET.SubElement(item, f"{{{ns}}}shortVersionString").text = version
    ET.SubElement(item, f"{{{ns}}}minimumSystemVersion").text = "12.0"
    ET.SubElement(item, "enclosure", {
        "url": f"{base}/{archive.name}", "length": str(archive.stat().st_size),
        "type": "application/octet-stream", f"{{{ns}}}edSignature": signature,
    })
    appcast = dist / "appcast-macos-arm64.xml"
    ET.ElementTree(rss).write(appcast, encoding="utf-8", xml_declaration=True)
    subprocess.run(
        [str(sparkle / "bin/sign_update"), "--ed-key-file", "-", str(appcast)],
        input=os.environ["IMMERSION_UPDATE_PRIVATE_KEY"].encode(), check=True,
    )


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--dist", type=Path, default=Path("dist"))
    parser.add_argument("--sparkle", type=Path, required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    publish(args.dist, args.version, args.sparkle)

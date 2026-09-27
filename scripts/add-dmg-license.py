#!/usr/bin/env python3
"""Attach the repository's GPLv3 license as a macOS disk image SLA."""

import base64
import plistlib
import subprocess
import sys
import tempfile
from pathlib import Path


repo = Path(__file__).resolve().parent.parent
license_data = (repo / "LICENSE").read_bytes()
template = (repo / "packaging/macos/eula-resources-template.xml").read_text()
resources = template.replace("${EULA_FORMAT}", "TEXT").replace(
    "${EULA_DATA}", base64.b64encode(license_data).decode("ascii")
)

with tempfile.TemporaryDirectory() as temporary:
    resource_file = Path(temporary) / "license.plist"
    resource_file.write_text(resources)
    subprocess.run(
        ["hdiutil", "udifrez", "-xml", str(resource_file), "", "-quiet", sys.argv[1]],
        check=True,
    )

embedded = plistlib.loads(
    subprocess.check_output(["hdiutil", "udifderez", "-xml", sys.argv[1]])
)
if embedded["TEXT"][0]["Data"] != license_data:
    raise RuntimeError("DMG license does not match the repository LICENSE")

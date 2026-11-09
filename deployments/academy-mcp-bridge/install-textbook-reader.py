#!/usr/bin/env python3
"""Install a pinned, separate PDF-reader interpreter. Does not activate an MCP.

An operator explicitly selects a new prefix and the reviewed wheel lock. Existing
prefixes are never replaced. No Academy credentials, request, mapping store or
client configuration is used. Intended for the native Linux x86_64 host.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import subprocess
import sys
import urllib.request
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--prefix', required=True, type=Path)
    parser.add_argument('--lock', required=True, type=Path)
    args = parser.parse_args()
    prefix = args.prefix
    if not prefix.is_absolute() or prefix.exists() or prefix.parent.resolve() != prefix.parent:
        raise ValueError('reader_install_prefix_rejected')
    if sys.platform != 'linux' or platform.machine() != 'x86_64' or sys.version_info < (3, 11):
        raise ValueError('reader_install_platform_unsupported')
    lock = json.loads(args.lock.read_text())
    if lock['package'] != 'PyMuPDF' or not lock['url'].startswith('https://files.pythonhosted.org/packages/'):
        raise ValueError('reader_install_lock_rejected')
    with urllib.request.urlopen(lock['url'], timeout=45) as response:
        wheel = response.read(128 * 1024 * 1024 + 1)
    if len(wheel) > 128 * 1024 * 1024 or hashlib.sha256(wheel).hexdigest() != lock['sha256']:
        raise ValueError('reader_install_wheel_rejected')
    subprocess.run([sys.executable, '-m', 'venv', '--copies', '--without-pip', str(prefix)], check=True)
    python = prefix / 'bin/python'
    site = Path(subprocess.check_output([str(python), '-I', '-c', 'import sysconfig;print(sysconfig.get_paths()["purelib"])'], text=True).strip())
    with zipfile.ZipFile(io.BytesIO(wheel)) as archive:
        for info in archive.infolist():
            if PurePosixPath(info.filename).is_absolute() or '..' in PurePosixPath(info.filename).parts or (info.external_attr >> 16) & 0o170000 == 0o120000:
                raise ValueError('reader_install_archive_rejected')
        archive.extractall(site)
    version = subprocess.check_output([str(python), '-I', '-B', '-c', 'import pymupdf;print(pymupdf.__version__)'], text=True).strip()
    if version != lock['version']:
        raise ValueError('reader_install_version_mismatch')
    # Libraries and interpreter must remain outside a writable source-owner
    # artifact directory. A deployment selects a root-controlled prefix.
    (prefix / 'wheel-lock.json').write_text(json.dumps(lock, indent=2) + '\n')
    for path in sorted(prefix.rglob('*'), reverse=True):
        if path.is_symlink():
            continue
        os.chmod(path, 0o555 if path.is_dir() or path.parent == prefix / 'bin' else 0o444)
    os.chmod(prefix, 0o555)
    print(json.dumps({'interpreter': str(python), 'sha256': hashlib.sha256(python.read_bytes()).hexdigest(), 'package': lock['package'], 'version': version}))


if __name__ == '__main__':
    main()

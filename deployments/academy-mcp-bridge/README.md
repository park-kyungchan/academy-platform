# Academy MCP bridge deployment sources

Byte copies captured on 2026-11-04; source hashes and file modes are recorded in `sources.json`. These are tracked deployment sources, not an installation or activation script.

| File | Installed host source |
| --- | --- |
| `academy-mcp-bridge.socket` | `/etc/systemd/system/academy-mcp-bridge.socket` |
| `academy-mcp-bridge@.service` | `/etc/systemd/system/academy-mcp-bridge@.service` |
| `launcher.mjs` | `/root/.local/share/academy-mcp-scoped-20261030/launcher.mjs` |
| `muse-academy` | `/root/.local/bin/muse-academy` |

The skill sources live in `content/skills/academy-runtime-routing/` and `content/skills/academy-student-context/`, copied from the corresponding `/root/.agents/skills/academy-*` directories. See `sources.json` for the complete file list.

Host paths in these originals are compatibility locators. The launcher still needs its separately owned release pointer, endpoints, bundles and credential/account-lock custody. Those runtime files are excluded. Skill references to host-specific tools remain source documentation. Copying these files grants no authority to run them, change registrations, deploy, reload systemd, or operate the Academy service. No host file was modified and no service/container action was performed.

The repository also contains `install-textbook-reader.py`, an explicitly invoked installer for the reviewed textbook PDF reader wheel/interpreter. It creates only a new operator-selected prefix and does not activate the MCP, change client registration or contact Academy. Its lock is `modules/academy-workbench/tools/academy-mcp/textbook-reader-runtime.json`; preserve installed dependency provenance and use the current native host release owner, not these historical bridge source copies, for activation.

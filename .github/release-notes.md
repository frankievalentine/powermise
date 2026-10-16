## Install

```sh
# macOS and Linux
curl -fsSL https://powermise.pages.dev/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://powermise.pages.dev/install.ps1 | iex
```

Or download an installer below:

| System | File |
| --- | --- |
| macOS (Apple Silicon and Intel) | `powermise-macos-universal.dmg` |
| Windows | `powermise-windows-x64-setup.exe` (or the `.msi`) |
| Linux x64 | `powermise-linux-x86_64.AppImage`, `-amd64.deb`, or `-x86_64.rpm` |
| Linux ARM | `powermise-linux-aarch64.AppImage`, `-arm64.deb`, or `-aarch64.rpm` |

`SHA256SUMS` lists every file's checksum.

## Opening it the first time

powermise isn't code-signed yet, so your system asks before opening it the first
time. The install script skips this on macOS; on Linux there's nothing to approve.

- **macOS** — if it says *“powermise” Not Opened*, click **Done**, then **Open
  Anyway** in **System Settings › Privacy & Security** (scroll to **Security**).
  Or run `xattr -dr com.apple.quarantine /Applications/powermise.app` in
  Terminal.
- **Windows** — if **Windows protected your PC** appears, click **More info**,
  then **Run anyway**.
- **Linux** — the `.deb` and `.rpm` install normally. An AppImage needs
  `chmod +x` and FUSE 2 (`libfuse2`, or `libfuse2t64` on Ubuntu 24.04).

More in [Getting started](https://powermise.pages.dev/getting-started/#opening-it-the-first-time).

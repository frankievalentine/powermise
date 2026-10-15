---
title: Targets & platform support
description: How powermise resolves harnesses across macOS, Linux, Windows, and WSL.
---

powermise is multi-target. Every resolution, preview, and convergence command
takes a `--target` that selects the platform providers used for tools, apps,
and native operations.

## Targets

| `--target` | Platform | Notes |
| --- | --- | --- |
| `darwin` | macOS (Apple Silicon & Intel) | Native operations use system package managers. |
| `linux` | Linux | Treated as a standard Linux environment. |
| `win32` | Windows | Native operations use Windows package managers. |
| `wsl` | Windows Subsystem for Linux | Treated as a **distinct Linux target**, not native Windows. |

`wsl` matters: a selection resolved for `wsl` uses Linux providers inside the
WSL environment, while `win32` uses native Windows providers. Pick the target
that matches where the tools will actually run.

## How providers are chosen

Each tool in the catalog declares a compatible mise provider per target
platform. When powermise resolves a selection it:

- selects the provider that matches the requested `--target`;
- reports an unsupported-provider error if no compatible provider exists;
- emits a deterministic configuration without package conflicts.

Native operations (for example a Windows package manager install, or a
verified install script) are separate from mise-managed tools. `plan` prints
the exact native command for each such operation in its native dry-run
section, so you can see precisely what a real convergence would invoke.

## Support matrix (current status)

| Target | Validation status |
| --- | --- |
| macOS (Apple Silicon) | End-to-end fixtures and CLI tests green; real clean-machine apply is an open acceptance gate |
| Linux | Fixture-validated; not yet clean-machine tested |
| Windows (`win32`) | Fixture-validated dry-runs only; no real Windows package installs are claimed until clean-machine evidence exists |
| WSL | Fixture target exists; treated as a distinct Linux target, not native Windows |

Before a target is marked "clean-machine verified" it must pass the acceptance
gates: a user-owned global mise config survives byte-for-byte, a generated
config is deterministic and atomically replaced, `plan` mutates nothing,
repeated `apply` converges, and every shipped asset has explicit platform and
ownership behavior.

## Resolving for a target

```sh
powermise selection show    --name my-agent --target darwin
powermise selection compile --selection my-agent.toml --target linux
powermise plan              --selection my-agent.toml --target wsl
powermise apply             --name my-agent --target win32 --yes
```

See [Commands](/ai-setups/commands/) for the full flag reference.

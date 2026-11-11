# UKSF tooling

Build and deployment tools for UKSF Arma 3 work. Each tool is a crate in this Cargo workspace, builds on Windows and Linux, and ships as a release binary.

## Tools

| Tool | Replaces | Use |
|---|---|---|
| `uksf-sign` | `DSCreateKey.exe`, `DSSignFile.exe` | `uksf-sign create-key <name>`, `uksf-sign sign <key.biprivatekey> <file.pbo>...`, `uksf-sign verify <key.bikey> <file.pbo>...` |
| `uksf-patcher` | the C# `uksf-patcher.exe` | `uksf-patcher apply --addons <dir> --patches <dir> --state <dir> [--dry-run]`, `uksf-patcher verify ...` with the same options, `uksf-patcher inspect`, `hash`, `p3d` |

`uksf-sign` writes V3 signatures. For addon PBOs they match `DSSignFile` byte for byte; `sign/tests/fixtures` holds the `DSSignFile` output it is tested against. `sign/examples/make_fixture.rs` rebuilds `sign_fixture.pbo`. The fixture key is a test key and signs nothing real.

## Releases

Push a `v*` tag. CI tests on Windows and Linux, then attaches `<tool>-linux-x64`, `<tool>-windows-x64.exe`, and `SHA256SUMS` to the GitHub release.

Licensed under GPL-2.0-only, because the tools link HEMTT crates.

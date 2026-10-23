# Windows, Linux and macOS

Remnary v0.2.1 builds native **x86-64 and ARM64 packages for all three operating systems**. Download a ZIP for Windows or tar.gz for Linux/macOS from [GitHub Releases](https://github.com/diocata/remnary/releases/latest). These are command-line applications with a browser dashboard, not GUI installers. Python/Rust are not required for normal operation of the downloaded binary; the optional demo script needs Python 3.11+.

## Build and runtime matrix

| Target | Native CI runner | Crypto / key storage |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | Windows Server 2022 x64 | Bundled OpenSSL; Windows Credential Manager or private key file |
| `aarch64-pc-windows-msvc` | Windows 11 ARM64 | Bundled OpenSSL; Windows Credential Manager or private key file |
| `x86_64-unknown-linux-gnu` | Ubuntu 22.04 x64 | Bundled OpenSSL; explicit private key file |
| `aarch64-unknown-linux-gnu` | Ubuntu 22.04 ARM64 | Bundled OpenSSL; explicit private key file |
| `x86_64-apple-darwin` | macOS 15 Intel | CommonCrypto; Keychain or private key file |
| `aarch64-apple-darwin` | macOS 26 Apple Silicon | CommonCrypto; Keychain or private key file |

A tag publishes only after every native job and the dependency advisory job pass. Each native job runs locked all-target tests, formatting, Clippy with warnings denied, an optimized build, executable smoke checks and target-specific license collection. `BUILD.json` records the actual runner, toolchain, source commit, target, executable hash and runtime-smoke result. [Workflow](../.github/workflows/ci.yml).

Linux binaries use glibc and are built on Ubuntu 22.04; the release baseline is **glibc 2.35+**. Alpine/musl, 32-bit CPUs, BSD and other triples are outside this matrix. macOS builds set a **13.0 deployment target**, with CI execution on macOS 15 (Intel) and 26 (Apple Silicon); the minimum linker target is not a claim that every macOS version was independently exercised. Local development was also tested on Apple Silicon macOS 27.0.1. Windows x64 CI runs on Server 2022; ARM64 runs on Windows 11. Other Windows versions are not independently covered by the runner matrix.

## Run a downloaded binary

Extract the archive into your own directory. On PowerShell:

```powershell
.\remnary.exe --version
.\remnary.exe init
.\remnary.exe import C:\path\to\conversations.json
.\remnary.exe analyze
.\remnary.exe web
```

Windows uses Credential Manager by default, scoped to the canonical archive location. The archive is under the user's local application-data directory unless `--data-dir` is supplied. Explicit private `--key-file` mode remains available. Keep key files in a directory protected by your own Windows ACL; Unix mode checks do not apply on Windows.

On macOS, use `./remnary` with the same subcommands. Native Keychain access can prompt. Linux intentionally requires an explicit private recovery-key file, avoiding a dependency on desktop D-Bus/Secret Service or a transient kernel keyring:

```sh
mkdir -p "$HOME/.config/remnary"
chmod 700 "$HOME/.config/remnary"
./remnary key create "$HOME/.config/remnary/recovery.key"
./remnary --key-file "$HOME/.config/remnary/recovery.key" init
./remnary --key-file "$HOME/.config/remnary/recovery.key" import /path/to/conversations.json
./remnary --key-file "$HOME/.config/remnary/recovery.key" analyze
./remnary --key-file "$HOME/.config/remnary/recovery.key" web
```

Create a key once; do not replace a key for an existing archive. Keep keys separate from backups. `--data-dir` selects a separate archive on every OS. Losing both the native credential and the recovery key loses access to encrypted data. [Recovery](recovery.md).

## Source builds

All targets use Rust **1.99.1**, the checked-in lockfile and a C compiler. macOS uses Xcode Command Line Tools and system CommonCrypto. Linux additionally needs `make`, Perl and the usual compiler/libc development tools (for example `build-essential` and `perl` on Ubuntu). Windows needs Visual Studio C++ build tools for the target architecture and native Windows Perl such as Strawberry Perl. NASM is optional for OpenSSL's optimized x64 assembly. The release recipe builds OpenSSL from locked source on Windows/Linux; no runtime OpenSSL DLL or database installation is required. Windows release builds use a static MSVC runtime.

The release recipe uses each target's native operating system and executes its output. It does not infer runtime support from a successful cross-compile. Test compilation can use the runner's normal C runtime; the packaged Windows binary is separately rebuilt with a static runtime and smoke-tested.

After installing the native prerequisites:

```sh
git clone https://github.com/diocata/remnary.git
cd remnary
cargo install --path . --locked
remnary --version
```

`rust-toolchain.toml` pins the tested toolchain. Put Cargo's binary directory on your `PATH`. To build without installing, run `cargo build --release --locked` and use `./target/release/remnary` (`.\target\release\remnary.exe` in PowerShell). The optional `scripts/cargo-local` helper uses this checkout's isolated toolchain when present and otherwise falls back to Cargo. For checked local packages, see [releasing](releasing.md).

## Signing and durability boundaries

macOS packages are ad-hoc signed and unnotarized. Windows packages do not have Authenticode signatures. No paid signing identity, public installer, update service or OS-security-policy modification is configured. Build from source if the unsigned distribution does not meet your environment's policies.

Filesystem tests cover managed-note updates, edits/conflicts, backup/restore and deletion cleanup on the native runners. Unix code synchronizes parent directories; Windows relies on completed file operations and its persistent SQL cleanup outbox. Native CI is not a hardware-power-loss guarantee. The separate SIGKILL recovery harness remains a local Unix engineering check, not a Windows power-loss test. [Readiness and historical evidence](readiness.md).

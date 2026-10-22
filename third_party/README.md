# Third-party materials

Remnary's own code is licensed under the root [MIT license](../LICENSE). Dependency licenses remain applicable to their own materials.

- `notices/` preserves the SQLCipher/source-crate/SQLite/rusqlite notices used by the native baseline. The LibTomCrypt notice is included for material in the source bundle; the macOS runtime selects CommonCrypto.
- `dependencies.json` records the locked package versions, source identities, declared licenses and collected notice paths for the `aarch64-apple-darwin` target.
- `rust-notices/` preserves notice files from the 153 reachable normal/build dependency source packages. Each package retains its original notice text and authorship.
- `sources/option-ext-0.2.0.crate` contains the unmodified source distribution of the MPL-2.0-covered dependency. Its license and source are provided alongside the executable distribution; Remnary does not modify that dependency.
- `rust-toolchain/` preserves the tested Rust toolchain's library copyright inventory and referenced license texts. The compiler/toolchain itself is not redistributed.

Regenerate the inventory with `python3 scripts/dependency-notices.py` after changing Cargo.lock or Rust. The script resolves the selected platform's normal/build graph, copies available notice files, includes the MPL source archive and fails if a package has no collected notice set. It does not scan user conversation archives. Review new license expressions and bundled native subcomponents when dependencies change: mechanically collecting files is not a legal certification.

Include this directory and the root LICENSE in any release archive. Checksum/source identifiers are recorded by Cargo.lock; the native engine provenance and build-provider limits are documented in [native-dependencies.md](../docs/native-dependencies.md). Other target platforms need their own inventory and runtime verification.

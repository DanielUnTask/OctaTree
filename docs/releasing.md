# Releasing OctaTree

Releases are built by GitHub Actions (`.github/workflows/release.yml`) when you push a version tag. The release contains one zip per platform, named the way Rokit expects, so the tool can be installed with `rokit add`.

## Release assets

| Asset | Platform |
|---|---|
| `octatree-<version>-windows-x86_64.zip` | Windows, 64-bit |
| `octatree-<version>-linux-x86_64.zip` | Linux, 64-bit |
| `octatree-<version>-linux-aarch64.zip` | Linux, ARM64 |
| `octatree-<version>-macos-x86_64.zip` | macOS, Intel |
| `octatree-<version>-macos-aarch64.zip` | macOS, Apple Silicon |
| `checksums.txt` | SHA-256 of every zip |

Each zip contains only `octatree` (or `octatree.exe`), `LICENSE` and `README.md`. Nothing from `examples/` or `tests/` is shipped.

## Making a release

1. Bump `version` in `Cargo.toml` (and run `cargo build` so `Cargo.lock` follows).
2. Make sure everything passes locally:

   ```powershell
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

3. Commit and push to `master`.
4. Tag the commit with the **same version, prefixed with `v`**, and push the tag:

   ```powershell
   git tag v0.1.0
   git push origin v0.1.0
   ```

5. Watch the *Release* workflow in the repository's *Actions* tab. It verifies that the tag matches `Cargo.toml`, runs fmt, clippy and the tests, builds all platforms, and publishes the release with generated notes. A tag containing `-` (for example `v0.2.0-beta.1`) is published as a pre-release.

If a build fails, fix the problem, delete the tag (`git tag -d v0.1.0` and `git push origin :refs/tags/v0.1.0`), and tag again.

## Installing a release with Rokit

```powershell
rokit add danieluntask/octatree
rokit install
```

This adds a line like `octatree = "danieluntask/octatree@0.1.0"` to your `rokit.toml`, after which `octatree` is available in that project. To pin or update a version, edit that line.

## Notes

- The workflow cannot be tested locally. The first tag is the first real run; use a pre-release tag (such as `v0.1.0-rc.1`) to try it without announcing anything.
- If a platform fails to build, you can remove its line from the matrix in `release.yml`; Rokit only needs the platforms your users run.

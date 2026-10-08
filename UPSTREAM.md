# Following CEF and cef-rs

tetsu is Kurogane's fork of [cef-rs](https://github.com/tauri-apps/cef-rs). cef-rs is a reference. tetsu never merges it. Fixes worth having are ported by hand and tetsu's bindings come from its own generator, run on CEF's headers.

## Last reviewed

cef-rs `f044f89` (2026-10-03, `154.4.0+154.0.33`), the commit tetsu forked from.

## Reviewing cef-rs

1. Fetch cef-rs and list its commits since the last one reviewed:

   ```sh
   git fetch https://github.com/tauri-apps/cef-rs dev
   git log --oneline f044f89..FETCH_HEAD
   ```

2. Read each one. Port what tetsu wants by hand, fixes to the hand-written code (`rc.rs`, `string.rs`, `args.rs`, the macros), to the generator (`update-bindings`) and to the tools. Skip cef-rs's binding regenerations; tetsu regenerates its own.
3. Record the commit reviewed above.

## Moving to a new CEF

1. `get-latest` finds the newest stable CEF published for every target. With `--update-version` it moves the workspace version, `<CEF major>.<minor>.<patch>+<CEF version>`. The scheduled `get-latest` workflow does this and opens a pull request against `dev`.
2. `update-bindings` regenerates the bindings for each of the eight targets. `cargo run -p update-bindings -- --bindgen --download --target <triple>` runs bindgen on CEF's headers, rewrites the sys bindings to call through the loaded libcef and generates the safe bindings and resources from them. The `update-bindings` workflow runs it on each platform.
3. tetsu's tests run (`cargo test --workspace`) and the cefsimple bundle runs on each platform (`cargo run -p bundle-cef-app -- cefsimple -o target/bundle`).
4. Kurogane's parity runs on the new tetsu decide its pin. Kurogane moves to a tetsu tag only once they pass; tetsu is tagged by hand, in pairs with Kurogane.

The generator emits tetsu's crate names, so a regeneration's diff is CEF's change alone. A patch to the bindings goes into the generator or the hand-written code, never into generated files alone.

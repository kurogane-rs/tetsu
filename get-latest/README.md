# get-latest

Finds the newest [Chromium Embedded Framework (CEF)](https://cef-builds.spotifycdn.com/index.html)
version published for every target tetsu supports. With `--update-version` it moves the workspace to
that version when it is newer, in tetsu's version scheme: `<CEF major>.<minor>.<patch>+<CEF version>`.

```sh
cargo run -p get-latest
```

```sh
cargo run -p get-latest -- --update-version
```

The bindings are regenerated afterwards with `update-bindings`, for every target. The scheduled
`get-latest` workflow does both and opens a pull request against `dev`; see `UPSTREAM.md`.

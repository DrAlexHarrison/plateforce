# The browser build

plateforce compiled to WebAssembly, wrapped in a static site. It is the whole product,
not a demonstration of it: the same compiled `plateforce-core` runs here as in the CLI.

**Nothing is uploaded.** A trace is read by the browser's file API, handed to WebAssembly
in the tab, and analysed there. There is no server, no request carrying sample data, and
no telemetry. That is the point rather than an implementation detail: subject force data
is re-identifiable, and the strongest privacy claim available is that it never moves.

## Build

```sh
./scripts/build-web.sh release     # or omit `release` for a fast debug build
python3 -m http.server -d web 8000
```

Open `http://localhost:8000`. Serving over http is required: browsers refuse to
instantiate a WebAssembly module from a `file://` URL.

The script installs the `wasm32-unknown-unknown` target and the exact `wasm-bindgen-cli`
version pinned in `crates/plateforce-wasm/Cargo.toml` if either is missing. The CLI
version and the crate version have to match, so the script reads the pin rather than
carrying a second copy of it.

## What is in here

| file | what it does |
|---|---|
| `index.html` | the document, and every empty, loading, error and fatal state |
| `styles.css` | the tokens, then the components. No webfont is fetched, because a font request is a request |
| `app.js` | loads the module, holds one trial, keeps the trace, the decisions and the numbers in agreement |
| `chart.js` | the trace, its landmarks, and the drag and keyboard interactions |
| `registry.js` | turns the registry into the decisions the interface presents |
| `pkg/` | build output, not in version control |

No bundler, no framework, no build step for the JavaScript. The files are served as
written, so the desktop wrapper can load the same directory with no server assumptions
and no absolute paths.

## No threads

The module is single threaded on purpose. Threads would need `SharedArrayBuffer`, which
would need `Cross-Origin-Opener-Policy` and `Cross-Origin-Embedder-Policy` headers, which
static hosting does not serve. A trial is 6,000 samples and a full recompute is
microseconds, so there is nothing to gain and a deployment channel to lose.

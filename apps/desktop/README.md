# Desktop application

This is the main GitButler application frontend code.
This has been built using Svelte, sweat and beers.

## Running the app on a browser (with but-server)

The application can be run using the dev server on a browser. This has to then access the rust end through the but-server.
The but-server provides the same API surface as the one provided by conventional Tauri.

### Steps

#### 1. Run the but-server

Execute the following command on your terminal \

```bash
cargo run -p but-server
```

This should start the server on th default port 6978

#### 2. Run the FE dev server

Execute the following command on another terminal, concurrently

```bash
pnpm dev:desktop-http
```

This packages the local workspace dependencies, builds the **web** target, points it to
the but-server on `http://localhost:6978` and serves it under the default address
`http://localhost:1420/`

If you run the desktop package's `dev` script directly, make sure the local package
outputs exist first:

```bash
pnpm turbo run package --filter=...@gitbutler/desktop
```

#### 3. Go to the browser

Open Chrome (let's not kid ourselves) and got to `http://localhost:1420` and enjoy

### Development

#### Auto-build the server on Rust changes

```bash
watchexec -w crates -r -- cargo run -p but-server
```

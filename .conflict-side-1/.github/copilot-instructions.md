## General information

This is a monorepo with multiple projects.
The main applications are found in the `apps` directory.
They are:

- `desktop` containing the Tauri application's frontend code
- `web` containing the web application's frontend code

The backend of the Tauri application is found in the `crates` directory.
It contains different rust packages, all used for the Tauri application.

The `packages` directory contains different self-contained npm packages.
These are shared between the `desktop` and `web` applications.
The packages are:

- `ui` containing the shared UI components
- `shared` containing the shared types and utils
- `no-relative-imports` containing the no-relative-imports ESLINT package

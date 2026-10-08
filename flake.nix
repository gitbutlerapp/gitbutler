{
  description = "GitButler development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Keep the browsers aligned with Playwright in pnpm-lock.yaml (1.58.2).
    nixpkgs-playwright.url = "github:NixOS/nixpkgs/7f6a6fb1c76e09426d6125e7e2543efe2a7f74e3";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    nixpkgs-playwright,
    flake-utils,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };

      playwrightBrowsers = nixpkgs-playwright.legacyPackages.${system}.playwright-driver.browsers;
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    in {
      devShells.default = pkgs.mkShell {
        packages = [
          rustToolchain
          pkgs.rust-analyzer
          pkgs.cargo-nextest
          pkgs.cargo-deny
          pkgs.cmake
          pkgs.curl
          pkgs.file
          pkgs.git
          pkgs.pkg-config
          pkgs.wget
          pkgs.nodejs_24
          pkgs.pnpm
          playwrightBrowsers
          pkgs.cargo-flamegraph
          pkgs.cargo-machete
        ];

        env = {
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        shellHook = ''
          # if we don't set TS_RS_EXPORT_DIR then `cargo test --all-features`
          # generates ts files and dirties the working copy
          export TS_RS_EXPORT_DIR="''${TMPDIR:-/tmp}/gitbutler-ts-rs"
          mkdir -p "$TS_RS_EXPORT_DIR"

          export PLAYWRIGHT_BROWSERS_PATH=${playwrightBrowsers}
        '';
      };
    });
}

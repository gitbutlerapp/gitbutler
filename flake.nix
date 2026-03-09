{
  description = "GitButler development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };

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
          pkgs.nodejs_22
          pkgs.pnpm
          pkgs.playwright-driver.browsers
        ];

        env = {
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        shellHook = ''
          # if we don't set TS_RS_EXPORT_DIR then `cargo test --all-features`
          # generates ts files and dirties the working copy
          export TS_RS_EXPORT_DIR="''${TMPDIR:-/tmp}/gitbutler-ts-rs"
          mkdir -p "$TS_RS_EXPORT_DIR"

          # We use different versions of Playwright in different packages... consider also
          # voidus/nix-playwright-browsers.
          export PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-driver.browsers}
        '';
      };
    });
}

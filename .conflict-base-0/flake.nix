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

      # Pin cargo-flamegraph to upstream main for macOS xctrace fixes that have
      # not been released yet.
      cargoFlamegraph = pkgs.rustPlatform.buildRustPackage (finalAttrs: {
        pname = "cargo-flamegraph";
        version = "0.6.12-unstable-2026-05-19";

        src = pkgs.fetchFromGitHub {
          owner = "flamegraph-rs";
          repo = "flamegraph";
          rev = "91bb0488920687168e3ccbb525e520f709ebc5c9";
          hash = "sha256-1yOYonN8douuiJQxtl2j2zBSlgdYVd46JGj7FJVSaHQ=";
        };

        cargoHash = "sha256-2T3nIhJt/npC2zr24HaAUvVCN04OFk1HSFoFk2lL+hI=";

        nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
          pkgs.makeWrapper
        ];

        postFixup = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
          wrapProgram $out/bin/cargo-flamegraph \
            --set-default PERF ${pkgs.perf}/bin/perf
          wrapProgram $out/bin/flamegraph \
            --set-default PERF ${pkgs.perf}/bin/perf
        '';
      });
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
          cargoFlamegraph
          pkgs.cargo-insta
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

          # We use different versions of Playwright in different packages... consider also
          # voidus/nix-playwright-browsers.
          export PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-driver.browsers}
        '';
      };
    });
}

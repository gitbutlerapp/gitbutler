# See also: https://github.com/gitbutlerapp/gitbutler/pull/11496
{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, flake-utils, nixpkgs, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay)(_: prev: {
          rsToolchain = prev.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        }) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      {
        devShells.default = with pkgs; mkShell {
          nativeBuildInputs = [
            cmake
            pnpm
            nodejs
            playwright-driver.browsers
            rsToolchain
            watchexec

            # For linking but-installer crate.
            curl
          ];

          shellHook = ''
            # We use different versions of Playwright in different packages... consider also
            # voidus/nix-playwright-browsers.
            export PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-driver.browsers}
          '';
        };
      }
    );
}

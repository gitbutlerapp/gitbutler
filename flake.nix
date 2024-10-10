{
  description = "Flake template";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fp.url = "github:hercules-ci/flake-parts";
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: inputs.fp.lib.mkFlake { inherit inputs; } {
    systems = inputs.nixpkgs.lib.systems.flakeExposed;

    perSystem = { system, config, pkgs, lib, ... }:
      {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = with inputs; [
            devshell.overlays.default
            rust-overlay.overlays.default
          ];
        };

        devShells.default = with pkgs; let
          nativeLibs = [
            at-spi2-atk
            cairo
            gdk-pixbuf
            glib
            gtk3
            harfbuzz
            libsoup
            pango
            webkitgtk
          ];
        in
        devshell.mkShell {
          imports = [ "${inputs.devshell}/extra/language/c.nix" ];

          language.c = {
            compiler = stdenv.cc;
            includes = nativeLibs;
            libraries = map (nl: nl.dev) nativeLibs;
          };

          packages = [
            (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
            cmake
            corepack_20
            gnumake
            nodejs_20
            pkg-config
          ];
        };
      };
  };
}

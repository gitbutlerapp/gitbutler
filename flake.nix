{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    unstablePkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    nix-playwright-browsers.url = "github:voidus/nix-playwright-browsers";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "unstablePkgs";
      };
    };
  };
  outputs = { self, unstablePkgs, nixpkgs, flake-utils, rust-overlay, nix-playwright-browsers }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          unstable = import unstablePkgs {
            inherit system overlays;
          };
          pkgs = import nixpkgs {
            system = system;
            overlays = [ nix-playwright-browsers.overlays.${system}.default ];
          };

          # Use stable.latest since rust-overlay doesn't always have exact versions like 1.91
          # The rust-toolchain.toml is used by rustup in the normal dev workflow
          rustToolchain = unstable.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "clippy" "rustfmt" ];
          };

          common = with pkgs; [
            gtk3
            glib
            glib-networking
            dbus
            openssl_3
            librsvg
            gettext
            libiconv
            libsoup
            libsoup_3
            webkitgtk
            webkitgtk_4_1
            nodejs_22
            corepack_22
            pkgs.playwright-browsers_v1_47_0
          ];

          # runtime Deps
          libraries = with pkgs; [
            cairo
            pango
            harfbuzz
            gdk-pixbuf
          ] ++ common;

          # compile-time deps
          packages = with pkgs; [
            curl
            wget
            pkg-config
            rustToolchain
          ] ++ common;
        in
        {
          devShells.default = pkgs.mkShell {
            nativeBuildInputs = packages;
            buildInputs = libraries;
            shellHook = ''
              LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
              XDG_DATA_DIRS=${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS
              GIO_MODULE_DIR=${pkgs.glib-networking.out}/lib/gio/modules/
              GIO_EXTRA_MODULES=${pkgs.glib-networking.out}/lib/gio/modules/

              PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-browsers_v1_47_0}
              PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
              PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
            '';
          };
        }
      );
}

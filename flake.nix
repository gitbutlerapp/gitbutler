{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    unstablePkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "unstablePkgs";
      };
    };
  };
  outputs = { self, unstablePkgs, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          unstable = import unstablePkgs {
            inherit system overlays;
          };
          pkgs = import nixpkgs {
            inherit system overlays;
          };

          rustToolchain = unstable.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

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
            nodejs_20
            corepack_20
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
          devShells.default = unstable.mkShell {
            nativeBuildInputs = packages;
            buildInputs = libraries;
            shellHook = ''
              export LD_LIBRARY_PATH=${unstable.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
              export XDG_DATA_DIRS=${unstable.gsettings-desktop-schemas}/share/gsettings-schemas/${unstable.gsettings-desktop-schemas.name}:${unstable.gtk3}/share/gsettings-schemas/${unstable.gtk3.name}:$XDG_DATA_DIRS
              export GIO_MODULE_DIR="${unstable.glib-networking}/lib/gio/modules/"
            '';
          };
        }
      );
}

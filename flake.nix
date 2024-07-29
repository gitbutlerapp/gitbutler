{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    unstable.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };
  outputs = { self, nixpkgs, unstable, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          # pkgs = import nixpkgs {
          #   inherit system overlays;
          # };
          unstablePkgs = import unstable {
            inherit system overlays;
          };

          rustToolchain = unstablePkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml.stable;

          common = with unstablePkgs; [
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
          libraries = with unstablePkgs;[
            cairo
            pango
            harfbuzz
            gdk-pixbuf
          ] ++ common;

          # compile-time deps
          packages = with unstablePkgs; [
            curl
            wget
            pkg-config
            rustToolchain
          ] ++ common;
        in
        with unstablePkgs;
        {
          devShells.default = mkShell {
            nativeBuildInputs = packages;
            buildInputs = libraries;
            shellHook = ''
              export LD_LIBRARY_PATH=${unstablePkgs.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
              export XDG_DATA_DIRS=${unstablePkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${unstablePkgs.gsettings-desktop-schemas.name}:${unstablePkgs.gtk3}/share/gsettings-schemas/${unstablePkgs.gtk3.name}:$XDG_DATA_DIRS
              export GIO_MODULE_DIR="${unstablePkgs.glib-networking}/lib/gio/modules/"
            '';
          };
        }
      );
}

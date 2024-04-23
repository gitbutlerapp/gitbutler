{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };

          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          common = with pkgs; [
            gtk3
            glib
            glib-networking
            dbus
            openssl_3
            librsvg
            libsoup_3
            webkitgtk
          ];

          # runtime Deps
          libraries = with pkgs;[
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
        with pkgs;
        {
          devShells.default = mkShell {
            # inherit buildInputs nativeBuildInputs;
            nativeBuildInputs = packages;
            buildInputs = libraries;

            # XDG_DATA_DIRS = let
            #   base = pkgs.lib.concatMapStringsSep ":" (x: "${x}/share") [
            #     pkgs.gnome.adwaita-icon-theme
            #     pkgs.shared-mime-info
            #   ];
            #
            #   gsettings_schema = pkgs.lib.concatMapStringsSep ":" (x: "${x}/share/gsettings-schemas/${x.name}") [
            #     pkgs.glib
            #     pkgs.gsettings-desktop-schemas
            #     pkgs.gtk3
            #   ];
            # in "${base}:${gsettings_schema}";
            shellHook =
              ''
                export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
                export XDG_DATA_DIRS=${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS
                export GIO_MODULE_DIR="${pkgs.glib-networking}/lib/gio/modules/"
              '';
          };
        }
      );
}

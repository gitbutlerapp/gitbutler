{
  description = "GitButler";

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
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };

      version = "0.0.0";

      # If you change Rust or pnpm dependencies, set these to `pkgs.lib.fakeHash` to have Nix print the expected hashes.
      cargoHash = "sha256-xwsAlCpeeAY0lWpAP0IB7o+MYc0/gcDq0bnVyvDPZLk=";
      pnpmHash = "sha256-03jmeeQdK27JKPUvlJPAWtBZVrO0K0SKhlN7Ml0xfWU=";

      commonRustAttrs = {
        inherit version cargoHash;
        src = self;

        nativeBuildInputs = [
          pkgs.cmake
          pkgs.pkg-config
        ];

        buildInputs = [
          pkgs.openssl
        ]
        ++ pkgs.lib.optional pkgs.stdenv.hostPlatform.isLinux pkgs.dbus;

        env = {
          OPENSSL_NO_VENDOR = true;
        };
      };

      but = rustPlatform.buildRustPackage (commonRustAttrs
        // {
          pname = "but";

          cargoBuildFlags = [
            "-p"
            "but"
            "--features"
            "packaged-but-distribution"
          ];

          doCheck = false;

          meta = {
            description = "GitButler command-line interface";
            homepage = "https://gitbutler.com";
            license = pkgs.lib.licenses.fsl11Mit;
            mainProgram = "but";
            platforms = pkgs.lib.platforms.linux ++ pkgs.lib.platforms.darwin;
          };
        });

      gitbutler = rustPlatform.buildRustPackage (finalAttrs:
        commonRustAttrs
        // {
          pname = "gitbutler";

          pnpmDeps = pkgs.fetchPnpmDeps {
            inherit (finalAttrs) pname version src;
            pnpm = pkgs.pnpm_10;
            fetcherVersion = 3;
            hash = pnpmHash;
          };

          postPatch = ''
            tauriConfig=crates/gitbutler-tauri/tauri.conf.release.json
            jq '
              .version = "${version}"
              | .bundle.createUpdaterArtifacts = false
              | .bundle.externalBin = ["gitbutler-git-askpass"]
            ' "$tauriConfig" > "$tauriConfig.tmp"
            mv "$tauriConfig.tmp" "$tauriConfig"

            substituteInPlace apps/desktop/src/lib/backend/tauri.ts \
              --replace-fail \
                'checkUpdate = tauriCheck;' \
                'checkUpdate = () => null;'
          '';

          nativeBuildInputs =
            commonRustAttrs.nativeBuildInputs
            ++ [
              pkgs.cacert
              pkgs.cargo-tauri.hook
              pkgs.desktop-file-utils
              pkgs.jq
              pkgs.nodejs_22
              pkgs.pnpmConfigHook
              pkgs.pnpm_10
              pkgs.turbo
              pkgs.wrapGAppsHook4
              pkgs.dart-sass
            ]
            ++ pkgs.lib.optional pkgs.stdenv.hostPlatform.isDarwin pkgs.makeBinaryWrapper;

          buildInputs =
            commonRustAttrs.buildInputs
            ++ pkgs.lib.optional pkgs.stdenv.hostPlatform.isDarwin pkgs.curl
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.glib-networking
              pkgs.webkitgtk_4_1
            ];

          tauriBuildFlags = [
            "--config"
            "crates/gitbutler-tauri/tauri.conf.release.json"
            "--features"
            "builtin-but,packaged-but-distribution,disable-auto-updates"
          ];

          env =
            commonRustAttrs.env
            // {
              CI = "true";
              COREPACK_ENABLE_STRICT = 0;
              RUSTFLAGS = "--cfg tokio_unstable";
              TRIPLE_OVERRIDE = pkgs.stdenv.hostPlatform.rust.rustcTarget;
              TURBO_BINARY_PATH = pkgs.lib.getExe pkgs.turbo;
              TURBO_TELEMETRY_DISABLED = 1;
            };

          preBuild = ''
            substituteInPlace \
              node_modules/.pnpm/sass-embedded@*/node_modules/sass-embedded/dist/lib/src/compiler-path.js \
              --replace-fail \
                'compilerCommand = (() => {' \
                'compilerCommand = (() => { return ["${pkgs.lib.getExe pkgs.dart-sass}"];'

            turbo run --filter @gitbutler/svelte-comment-injector build
            pnpm build:desktop -- --mode production
          '';

          postInstall =
            pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
              makeBinaryWrapper \
                "$out/Applications/GitButler.app/Contents/MacOS/gitbutler-tauri" \
                "$out/bin/gitbutler-tauri"
            ''
            + pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
              desktop-file-edit \
                --set-comment "A Git client for simultaneous branches on top of your existing workflow." \
                --set-key Keywords \
                --set-value "git;" \
                --set-key StartupWMClass \
                --set-value GitButler \
                "$out/share/applications/GitButler.desktop"
            '';

          doCheck = false;

          meta = {
            description = "Git client for simultaneous branches";
            homepage = "https://gitbutler.com";
            license = pkgs.lib.licenses.fsl11Mit;
            mainProgram = "gitbutler-tauri";
            platforms = pkgs.lib.platforms.linux ++ pkgs.lib.platforms.darwin;
          };
        });

      gitbutlerWithCli = pkgs.symlinkJoin {
        name = "gitbutler-with-cli-${version}";
        paths = [
          gitbutler
          but
        ];
        meta.mainProgram = "gitbutler-tauri";
      };

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
      packages = {
        inherit gitbutler but;
        default = gitbutlerWithCli;
      };

      apps = {
        gitbutler = flake-utils.lib.mkApp {
          drv = gitbutler;
        };
        but = flake-utils.lib.mkApp {
          drv = but;
        };
        default = flake-utils.lib.mkApp {
          drv = gitbutler;
        };
      };

      checks = {
        inherit gitbutler but;
      };

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

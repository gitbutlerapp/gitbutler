# Installing GitButler with Nix

GitButler can be consumed as a flake input:

```nix
{
  inputs.gitbutler.url = "github:gitbutlerapp/gitbutler";

  outputs = {nixpkgs, gitbutler, ...}: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    devShells.${system}.default = pkgs.mkShell {
      packages = [
        # Installs both the GitButler GUI and the `but` CLI.
        gitbutler.packages.${system}.default
      ];
    };
  };
}
```

The packages are also available separately:

- `packages.<system>.gitbutler` provides the GUI.
- `packages.<system>.but` provides the CLI.
- `packages.<system>.default` provides both.

If you change Rust or pnpm dependencies, you'll need to update the fixed-output hashes in `flake.nix`.
Set `cargoHash` (for `.#but`) or `pnpmHash` (for `.#gitbutler`) to `pkgs.lib.fakeHash`, then run the corresponding `nix build` and copy the expected hash that Nix prints.

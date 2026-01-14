{
  description = "typ2anki";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = inputs: {
    packages = builtins.mapAttrs (
      system: _:
      let
        pkgs = inputs.nixpkgs.legacyPackages.${system};
      in
      {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "typ2anki";
          version = "1.0.9";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.rustPlatform.bindgenHook
          ];

          buildInputs = [
            pkgs.openssl
          ];
        };
      }
    ) inputs.nixpkgs.legacyPackages;

    devShells = builtins.mapAttrs (
      system: _:
      let
        pkgs = inputs.nixpkgs.legacyPackages.${system};
      in
      {
        default = pkgs.mkShell {
          name = "typ2anki";
          packages = [
            inputs.self.packages.${system}.default
            pkgs.typst
            pkgs.rustc
            pkgs.cargo
            pkgs.rust-analyzer
            pkgs.clippy
          ];
          shellHook = ''
            echo "Welcome to typ2anki (Rust version) shell"
            typ2anki --version
            typst --version
          '';
        };
      }
    ) inputs.nixpkgs.legacyPackages;
  };
}

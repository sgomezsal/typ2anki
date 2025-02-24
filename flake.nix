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
        default = pkgs.python3.pkgs.buildPythonApplication {
          pname = "typ2anki";
          version = "unstable-2025-01-08";
          pyproject = true;

          src = ./.;

          build-system = [
            pkgs.python3.pkgs.setuptools
          ];

          dependencies = [
            pkgs.python3.pkgs.requests
            pkgs.typst
          ];

          pythonImportsCheck = [
            "typ2anki"
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
            pkgs.pylint
            pkgs.python313Packages.python-lsp-server
            pkgs.black
          ];
          shellHook = # bash
            ''
              echo "Welecome to typ2anki shell"
              which typ2anki
              typst --version
            '';
        };
      }
    ) inputs.nixpkgs.legacyPackages;
  };
}

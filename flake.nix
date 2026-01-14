{
  description = "typ2anki";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay, ... } @ inputs: 
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; };
          
          # Obtenemos la versión de Rust que soporta Edition 2024
          rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);

          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
        in
        {
          default = rustPlatform.buildRustPackage {
            pname = "typ2anki";
            version = "1.0.9";
            src = ./typ2anki-rust;

            cargoLock = {
              lockFile = ./typ2anki-rust/Cargo.lock;
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
      );

      devShells = forAllSystems (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; };
          rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
        in
        {
          default = pkgs.mkShell {
            name = "typ2anki";
            packages = [
              self.packages.${system}.default
              pkgs.typst
              rustToolchain
              pkgs.rust-analyzer
            ];
            shellHook = ''
              echo "Welcome to typ2anki (Rust Edition 2024) shell"
              typst --version
            '';
          };
        }
      );
    };
}

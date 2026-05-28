{
  description = "Zed extension for .http files (with httpyac-backed LSP)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ { flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      perSystem = { pkgs, system, ... }:
        let
          pkgsWithRust = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };
          rust = pkgsWithRust.rust-bin.stable.latest.default.override {
            targets = [ "wasm32-wasip2" "wasm32-wasip1" "wasm32-unknown-unknown" ];
          };
          rustPlatform = pkgsWithRust.makeRustPlatform {
            cargo = rust;
            rustc = rust;
          };
          cargoToml = builtins.fromTOML (builtins.readFile ./http-lsp/Cargo.toml);

          zed-http-lsp = rustPlatform.buildRustPackage {
            pname = "zed-http-lsp";
            version = cargoToml.package.version;

            src = pkgs.lib.cleanSourceWith {
              src = ./.;
              filter = path: type:
                let
                  rel = pkgs.lib.removePrefix (toString ./. + "/") (toString path);
                in
                !(pkgs.lib.hasPrefix "target" rel
                  || pkgs.lib.hasPrefix "grammars" rel
                  || pkgs.lib.hasSuffix ".wasm" rel
                  || pkgs.lib.hasPrefix ".direnv" rel);
            };

            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "zed-http-lsp" ];
            cargoTestFlags = [ "-p" "zed-http-lsp" "-p" "httpyac-rs" ];

            meta = with pkgs.lib; {
              description = "LSP for .http files that delegates execution to httpyac";
              homepage = "https://github.com/ToyVoDev/zed-http";
              license = licenses.asl20;
              mainProgram = "zed-http-lsp";
            };
          };
        in
        {
          packages = {
            inherit zed-http-lsp;
            default = zed-http-lsp;
          };

          devShells.default = pkgsWithRust.mkShell {
            packages = [
              rust
              pkgs.tree-sitter
              pkgs.nodejs
              pkgs.clang
              pkgs.httpyac
            ];

            shellHook = ''
              echo "zed-http dev shell — rust $(rustc --version | cut -d' ' -f2), tree-sitter $(tree-sitter --version | cut -d' ' -f2), httpyac $(httpyac --version 2>/dev/null || echo missing)"
            '';
          };

          formatter = pkgs.nixfmt-rfc-style;
        };
    };
}

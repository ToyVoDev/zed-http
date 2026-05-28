{
  description = "Zed extension for .http files";

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
        in
        {
          devShells.default = pkgsWithRust.mkShell {
            packages = [
              rust
              pkgs.tree-sitter
              pkgs.nodejs
              pkgs.clang
            ];

            shellHook = ''
              echo "zed-http dev shell — rust $(rustc --version | cut -d' ' -f2), tree-sitter $(tree-sitter --version | cut -d' ' -f2)"
            '';
          };

          formatter = pkgs.nixfmt-rfc-style;
        };
    };
}

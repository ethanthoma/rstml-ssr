{
  description = "Minimal server-side HTML rendering with rstml";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      for_all_systems = nixpkgs.lib.genAttrs systems;
      packages_for = system: import nixpkgs { inherit system; };
    in
    {
      packages = for_all_systems (
        system:
        let
          pkgs = packages_for system;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "rstml-ssr";
            version = "0.1.0";
            src = pkgs.lib.cleanSource self;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "--workspace" ];
            cargoTestFlags = [ "--workspace" ];
          };
        }
      );

      checks = for_all_systems (
        system:
        let
          pkgs = packages_for system;
        in
        {
          build = self.packages.${system}.default;
          policy = pkgs.runCommand "rstml-ssr-policy" { nativeBuildInputs = [ pkgs.ast-grep ]; } ''
            cd ${self}
            ast-grep test --config .ast-grep/sgconfig.yml --skip-snapshot-tests
            ast-grep scan --config .ast-grep/sgconfig.yml crates
            touch "$out"
          '';
          formatting =
            pkgs.runCommand "rstml-ssr-formatting"
              {
                nativeBuildInputs = [
                  pkgs.cargo
                  pkgs.rustfmt
                  pkgs.leptosfmt
                  pkgs.nixfmt
                  pkgs.python3Packages.mdformat
                  pkgs.biome
                  pkgs.yamlfmt
                ];
              }
              ''
                cp -r ${self} source
                chmod -R u+w source
                cd source
                cargo fmt --all -- --check
                leptosfmt --check --config-file leptosfmt.toml \
                  'crates/rstml-ssr/src/*.rs' 'crates/rstml-ssr/tests/*.rs' \
                  crates/rstml-ssr/tests/fixtures/rstml_fixture.rs \
                  crates/rstml-ssr/tests/pass/composition.rs
                nixfmt --check flake.nix
                mdformat --check --wrap 80 --number README.md crates/rstml-ssr-macros/README.md crates/rstml-ssr/tests/fixtures/README.md
                biome format crates/rstml-ssr/tests/fixtures/browser-checks.js
                yamlfmt -lint .ast-grep
                touch "$out"
              '';
        }
      );

      devShells = for_all_systems (
        system:
        let
          pkgs = packages_for system;
        in
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.rustfmt
              pkgs.leptosfmt
              pkgs.secretspec
              pkgs.clippy
              pkgs.ast-grep
              pkgs.rust-analyzer
              pkgs.nixfmt
              pkgs.python3Packages.mdformat
              pkgs.biome
              pkgs.yamlfmt
            ];
          };
          browser = pkgs.mkShell {
            inputsFrom = [ self.devShells.${system}.default ];
            packages = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.chromium ];
          };
        }
      );

      formatter = for_all_systems (system: (packages_for system).nixfmt);
    };
}

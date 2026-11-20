{
  description = "Minimal server-side HTML rendering with rstml";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      self,
      flake-parts,
      devshell,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ devshell.flakeModule ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      perSystem =
        {
          pkgs,
          self',
          ...
        }:
        let
          rust_package = {
            pname = "rstml-ssr";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
            src = pkgs.lib.cleanSource self;
            cargoLock.lockFile = ./Cargo.lock;
          };
          devshell_packages = [
            pkgs.stdenv.cc
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.leptosfmt
            pkgs.secretspec
            pkgs.git
            pkgs.gh
            pkgs.actionlint
            pkgs.ripgrep
            pkgs.clippy
            pkgs.ast-grep
            pkgs.rust-analyzer
            pkgs.nixfmt
            pkgs.python3Packages.mdformat
            pkgs.biome
            pkgs.yamlfmt
            pkgs.taplo
          ];
        in
        {
          packages.default = pkgs.rustPlatform.buildRustPackage (
            rust_package
            // {
              cargoBuildFlags = [ "--workspace" ];
              cargoTestFlags = [ "--workspace" ];
            }
          );

          checks = {
            build = self'.packages.default;
            clippy = pkgs.rustPlatform.buildRustPackage (
              rust_package
              // {
                pname = "rstml-ssr-clippy";
                nativeBuildInputs = [ pkgs.clippy ];
                buildPhase = ''
                  runHook preBuild
                  cargo clippy --offline --workspace --all-targets -- -D warnings
                  runHook postBuild
                '';
                doCheck = false;
                installPhase = ''
                  touch "$out"
                '';
              }
            );
            rustdoc = pkgs.rustPlatform.buildRustPackage (
              rust_package
              // {
                pname = "rstml-ssr-rustdoc";
                buildPhase = ''
                  runHook preBuild
                  RUSTDOCFLAGS="-Dwarnings" cargo doc --offline --workspace --no-deps
                  runHook postBuild
                '';
                doCheck = false;
                installPhase = ''
                  touch "$out"
                '';
              }
            );
            policy = pkgs.runCommand "rstml-ssr-policy" { nativeBuildInputs = [ pkgs.ast-grep ]; } ''
              cd ${self}
              ast-grep test --config .ast-grep/sgconfig.yml --skip-snapshot-tests
              ast-grep scan --config .ast-grep/sgconfig.yml crates
              touch "$out"
            '';
            readme =
              let
                runtime = builtins.fromTOML (builtins.readFile ./crates/rstml-ssr/Cargo.toml);
                macros = builtins.fromTOML (builtins.readFile ./crates/rstml-ssr-macros/Cargo.toml);
              in
              assert runtime.package.readme == "../../README.md";
              assert macros.package.readme == "README.md";
              pkgs.runCommand "rstml-ssr-package-readme" { } ''
                test -s ${self}/README.md
                test -s ${self}/crates/rstml-ssr-macros/README.md
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
                    pkgs.taplo
                    pkgs.actionlint
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
                    crates/rstml-ssr/tests/fixtures/renamed.rs \
                    crates/rstml-ssr/tests/pass/composition.rs \
                    crates/rstml-ssr-macros/src/tests.rs
                  nixfmt --check flake.nix
                  mdformat --check --wrap 80 --number README.md crates/rstml-ssr-macros/README.md crates/rstml-ssr/tests/fixtures/README.md
                  biome format biome.json crates/rstml-ssr/tests/fixtures/browser-checks.js
                  yamlfmt -lint .ast-grep
                  yamlfmt -lint .github/workflows
                  yamlfmt -lint .yamlfmt
                  taplo fmt --check Cargo.toml crates/rstml-ssr/Cargo.toml crates/rstml-ssr-macros/Cargo.toml leptosfmt.toml rustfmt.toml secretspec.toml taplo.toml
                  actionlint .github/workflows/*.yml
                  touch "$out"
                '';
          };

          devshells = {
            default = {
              devshell = {
                name = "rstml-ssr";
                packages = devshell_packages;
              };
            };
            browser = {
              devshell = {
                name = "rstml-ssr-browser";
                packages =
                  devshell_packages ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.chromium ];
              };
            };
          };

          formatter = pkgs.nixfmt;
        };
    };
}

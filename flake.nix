# Build, testing, and developments specification for the `nix` environment
#
# # Prerequisites
#
# - Install the `nix` package manager: https://nixos.org/download/
# - Configure `flake` support: https://nixos.wiki/wiki/Flakes
#
# # Build
#
# ```
# $ nix build --print-build-logs
# ```
#
# This produces:
#
# - ./result/bin/zebra-scanner
# - ./result/bin/zebrad-for-scanner
# - ./result/bin/zebrad
# - ./result/book/
#
# The book directory is the root of the book source, so to view the rendered book:
#
# ```
# $ xdg-open ./result/book/book/index.html
# ```
#
# # Development
#
# ```
# $ nix develop
# ```
#
# This starts a new subshell with a development environment, such as
# `cargo`, `clang`, `protoc`, etc... So `cargo test` for example should
# work.
{
  description = "The zebra zcash node binaries and crates with Crosslink protocol features";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # TODO: Switch to `flake-parts` lib for cleaner organization.
    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    inputs:
    inputs.flake-utils.lib.eachDefaultSystem (
      system:
      let
        project-name = "zebra-crosslink";

        # Local utility library:
        flakelib = import ./flake inputs {
          pname = "${project-name}-workspace";
          src-root = ./.;
          rust-toolchain-toml = ./rust-toolchain.toml;
          inherit system;
        };

        inherit (flakelib)
          build-rust-workspace
          links-table
          nixpkgs
          run-command
          select-source
          legacy-select-source
          ;

        # We use this style of nix formatting in checks and the dev shell:
        nixfmt = nixpkgs.nixfmt-rfc-style;

        # We use the latest nixpkgs `libclang`:
        inherit (nixpkgs.llvmPackages) libclang;

        src-book = select-source {
          name-suffix = "book";
          paths = [
            ./book
            ./CONTRIBUTING.md
            ./DELIVERABLES.md
            ./README.md
            ./SCOPING.md
          ];
        };

        src-rust = select-source {
          name-suffix = "rust";
          paths = [
            ./.cargo
            ./.config
            ./Cargo.lock
            ./Cargo.toml
            ./checked_in_sl_test_net_genesis.hex
            ./clippy.toml
            ./codecov.yml
            ./command_to_submit_test_net_genesis.txt
            ./crosslink-test-data
            ./deny.toml
            ./firebase.json
            ./grafana
            ./katex-header.html
            ./LICENSE-APACHE
            ./LICENSE-MIT
            ./openapi.yaml
            ./prometheus.yaml
            ./release.toml
            ./rust-toolchain.toml
            ./supply-chain
            ./tower-batch-control
            ./tower-fallback
            ./vibe_coded_script_to_create_a_test_net_genesis.sh
            ./zebra-chain
            ./zebra-consensus
            ./zebra-crosslink
            ./zebra-grpc
            ./zebra-network
            ./zebra-node-services
            ./zebra-rpc
            ./zebra-scan
            ./zebra-script
            ./zebra-state
            ./zebra-test
            ./zebra-utils
            ./zebrad
          ];
        };

        src-rust-legacy = legacy-select-source {
          prune = [
            "book"
            "docker"
            "docs"
            "flake"
            "grafana"
            "supply-chain"
          ];

          # Include all files in these directories, even if not "typical rust":
          include = [
            # These have things besides `*.toml` or `*.rs` we need:
            "crosslink-test-data"
            "zebra-chain"
            "zebra-consensus"
            "zebra-crosslink"
            "zebra-rpc"
            "zebra-test"
            "zebrad"
          ];
        };

        zebrad-outputs = build-rust-workspace ./zebrad {
          src = src-rust;

          strictDeps = true;

          # Note: we disable tests since we'll run them all via cargo-nextest
          doCheck = false;

          # Use the clang stdenv, overriding any downstream attempt to alter it:
          stdenv = _: nixpkgs.llvmPackages.stdenv;

          nativeBuildInputs = with nixpkgs; [
            pkg-config
            protobuf
          ];

          buildInputs = with nixpkgs; [
            libclang
            rocksdb
          ];

          # Additional environment variables can be set directly
          LIBCLANG_PATH = "${libclang.lib}/lib";
        };

        zebrad = zebrad-outputs.pkg;

        zebra-book = nixpkgs.stdenv.mkDerivation rec {
          name = "zebra-book";
          src = src-book;
          buildInputs = with nixpkgs; [
            mdbook
            mdbook-mermaid
          ];
          builder = nixpkgs.writeShellScript "${name}-builder.sh" ''
            if mdbook build --dest-dir "$out/book/book" "$src/book" 2>&1 | grep -E 'ERROR|WARN'
            then
              echo 'Failing due to mdbook errors/warnings.'
              exit 1
            fi
          '';
        };

        # FIXME: Replace this with `select-source` and `nixpkgs.linkFarm`:
        storepath-to-derivation =
          src:
          let
            inherit (builtins) baseNameOf head match;
            inherit (nixpkgs.lib.strings) isStorePath;

            srcName = if isStorePath src then head (match "^[^-]+-(.*)$" src) else baseNameOf src;
          in

          nixpkgs.stdenv.mkDerivation rec {
            inherit src;

            name = "ln-to-${srcName}";

            builder = nixpkgs.writeShellScript "script-to-${name}" ''
              outsrc="$out/src"
              mkdir -p "$outsrc"
              ln -sv "$src" "$outsrc/${srcName}"
            '';

          };

        src-delta = run-command "src-diff" [ ] ''
          echo 'diffing old/new src-rust...'
          if ! diff -r '${src-rust-legacy}' '${src-rust}' > "$out"
          then
            echo "REGRESSION: The new rust source selection differs from 'main'-branch, see: $out"
            exit 1
          fi
        '';
      in
      {
        packages = (
          let
            base-pkgs = {
              inherit
                zebrad
                zebra-book
                src-book
                src-rust
                src-delta
                ;

              src-rust-legacy = storepath-to-derivation src-rust-legacy;
            };

            all = links-table "all" {
              "./bin" = "${zebrad}/bin";
              "./book" = "${zebra-book}/book";
              "./src/${project-name}/book" = "${src-book}/book";
              "./src/${project-name}/rust" = src-rust;
              "./src/${project-name}/delta" = src-delta;
            };
          in

          base-pkgs
          // {
            inherit all;
            default = all;
          }
        );

        checks = (
          zebrad-outputs.checks
          // {
            # Build the crates as part of `nix flake check` for convenience
            inherit zebrad;

            # Check formatting
            nixfmt-check = run-command "nixfmt" [ nixfmt ] ''
              set -efuo pipefail
              exitcode=0
              for f in $(find '${./.}' -type f -name '*.nix')
              do
                cmd="nixfmt --check --strict \"$f\""
                echo "+ $cmd"
                eval "$cmd" || exitcode=1
              done
              [ "$exitcode" -eq 0 ] && touch "$out" # signal success to nix
              exit "$exitcode"
            '';
          }
        );

        apps = {
          zebrad = inputs.flake-utils.lib.mkApp { drv = zebrad; };
        };

        # TODO: BEWARE: This dev shell may have buggy deviations from the build.
        devShells.default = (
          let
            mkClangShell = nixpkgs.mkShell.override { inherit (nixpkgs.llvmPackages) stdenv; };

            devShellInputs = with nixpkgs; [
              rustup
              mdbook
              mdbook-mermaid
              nixfmt
              yamllint
            ];

            dynlibs = with nixpkgs; [
              libGL
              libxkbcommon
              xorg.libX11
              xorg.libxcb
              xorg.libXi
            ];

            crate-args = zebrad-outputs.args.crate;
          in
          mkClangShell (
            crate-args
            // {
              # Include devShell inputs:
              nativeBuildInputs = crate-args.nativeBuildInputs ++ devShellInputs;

              LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath dynlibs;
            }
          )
        );
      }
    );
}

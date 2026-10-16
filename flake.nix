{
  description = "clipper - discord voice clip bot";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            pkg-config
            cmake
            libopus
          ];

          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

          # the prebuilt libmoonshine (scripts/fetch-moonshine.sh) needs libstdc++ at runtime
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc.lib ];

          shellHook = ''
            if [ -f .env ]; then
              set -a; . ./.env; set +a
            fi
          '';
        };
      });
    };
}

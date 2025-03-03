{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, naersk, ... }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs { inherit system; };
      naersk-lib = pkgs.callPackage naersk { };
    in {
      packages.default = naersk-lib.buildPackage ./.;

      devShells.default = with pkgs; mkShell {
        buildInputs = [ cargo rustc rustfmt rustPackages.clippy cargo-watch ];
        RUST_SRC_PATH = rustPlatform.rustLibSrc;
      };
    });
}

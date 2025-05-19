{
  description = "nockchain flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, naersk }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;

          overlays = [
            (import rust-overlay)
          ];
        };
        lib = pkgs.lib;

        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        naersk' = pkgs.callPackage naersk {
          cargo = toolchain;
          rustc = toolchain;
        };
        code = pkgs.callPackage ./nix/. { inherit pkgs system naersk' lib; };
      in rec {
        packages = {
          hoonc = code.hoonc;
          nockchain = code.nockchain;
          nockchain-wallet = code.nockchain-wallet;
          all = pkgs.symlinkJoin {
            name = "all";
            paths = with code; [ hoonc nockchain nockchain-wallet ];
          };
          default = packages.all;
        };
        defaultPackage = packages.default;
      }
    );
}

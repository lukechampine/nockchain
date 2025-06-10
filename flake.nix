{
  description = "nockchain flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;

          overlays = [
            (import rust-overlay)
          ];
        };
        lib = pkgs.lib;

        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p: p.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml
        );

        code = pkgs.callPackage ./nix/. { inherit pkgs system lib craneLib; };
      in rec {
        packages = code // {
          all = pkgs.symlinkJoin {
            name = "all";
            paths = with code; [ hoonc nockchain nockchain-wallet nockchain-metrics-exporter ];
          };
          default = packages.all;
        };
        defaultPackage = packages.default;
      }
    );
}

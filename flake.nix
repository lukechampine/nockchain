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
    flake-utils.lib.eachDefaultSystem (localSystem:
      let
        pkgs = (import nixpkgs) {
          inherit localSystem;

          overlays = [
            (import rust-overlay)
          ];
        };

        code = crossPkgs: let
          lib = crossPkgs.lib;
          rustToolchainFor = p: p.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (crane.mkLib crossPkgs).overrideToolchain rustToolchainFor;
          localCraneLib = (crane.mkLib pkgs).overrideToolchain rustToolchainFor;
        in
          crossPkgs.callPackage ./nix/. { inherit localSystem lib craneLib localCraneLib rustToolchainFor; pkgs = crossPkgs; localPkgs = pkgs; };

        localCode = code pkgs;
      in rec {
        packages = localCode // localCode.nbx-internal // {
          all = pkgs.symlinkJoin {
            name = "all";
            paths = with localCode; [ hoonc nockchain nockchain-wallet nockchain-metrics-exporter ];
          };
          cross = pkgs.lib.mapAttrs (n: v: code v) pkgs.pkgsCross;
          default = packages.all;
        };
        defaultPackage = packages.default;
      }
    );
}

{ stdenv, pkgs, lib, craneLib, ... }:
let
  base = pkgs.callPackage ./base.nix { inherit pkgs lib; };
  hoonc = pkgs.callPackage ./hoonc.nix { inherit stdenv lib base craneLib commonArgs; };
  jam-pkg = pkgs.callPackage ./jam.nix { inherit base hoonc; };

  src = base.noNix ../.;
  commonArgs = {
    inherit src;
    strictDeps = true;
    pname = "nockchain-deps";
    # Additional environment variables can be set directly
    SHADERC_LIB_DIR="${pkgs.shaderc.static}/lib";
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  individualCrateArgs = commonArgs // {
    inherit cargoArtifacts;
    inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
    # NB: we disable tests since we'll run them all via cargo-nextest
    doCheck = false;
  };

  nockchain-base = profile: extraArgs: craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain";
    CARGO_PROFILE = profile;
    cargoExtraArgs = "-p nockchain --features nockchain/jemalloc ${extraArgs}";
    buildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.dumb-jam.out} './assets/dumb.jam' && cp ${jam-pkg.miner-jam.out} './assets/miner.jam'";
  });

  wallet-base = craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain-wallet";
    cargoExtraArgs = "-p nockchain-wallet";
    nativeBuildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.wallet-jam.out} './assets/wal.jam'";
  });

  metrics-exporter-base = craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain-metrics-exporter";
    cargoExtraArgs = "-p nockchain-metrics-exporter";
    nativeBuildInputs = [ ];
  });

  nockchain = extraArgs: (nockchain-base "release" extraArgs);
  nockchain-v4 = extraArgs: if lib.strings.hasInfix "x86_64-" pkgs.system then (nockchain-base "release-v4" extraArgs) else throw "release-v4 is only supported on x86_64 targets!";
  makeGpu = call: call "--features nockchain/gpu --features nbx-jetpack/gpu-prod";
in
{
  hoonc = hoonc.hoonc;
  nockchain = nockchain "";
  nockchain-gpu = makeGpu nockchain;
  nockchain-v4 = nockchain-v4 "";
  nockchain-v4-gpu = makeGpu nockchain-v4;
  nockchain-native = (nockchain-base "release-native");
  nockchain-wallet = wallet-base;
  nockchain-metrics-exporter = metrics-exporter-base;
  nockchain-jamfiles = jam-pkg;
}

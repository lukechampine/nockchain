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

  nbx-miner-base = profile: extraArgs: craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nbx-miner";
    CARGO_PROFILE = profile;
    cargoExtraArgs = "-p nbx-miner --features nbx-miner/jemalloc ${extraArgs}";
    buildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.miner-jam.out} './assets/miner.jam'";
  });

  profile-v = v: if lib.strings.hasInfix "x86_64-" pkgs.system then "release-v${v}" else throw "release-v${v} is only supported on x86_64 targets!";
  profile-v4 = profile-v "4";
  profile-v3 = profile-v "3";

  nockchain = extraArgs: (nockchain-base "release" extraArgs);
  nockchain-v4 = extraArgs: (nockchain-base profile-v4 extraArgs);
  nockchain-v3 = extraArgs: (nockchain-base profile-v3 extraArgs);

  nbx-miner = extraArgs: (nbx-miner-base "release" extraArgs);
  nbx-miner-v4 = extraArgs: (nbx-miner-base profile-v4 extraArgs);
  nbx-miner-v3 = extraArgs: (nbx-miner-base profile-v3 extraArgs);

  makeGpu = call: call "--features nbx-miner/gpu --features nbx-jetpack/gpu-prod --features nbx-miner/prom-exporter";
  makeStealthGpu = call: call "--features nbx-miner/gpu --features nbx-jetpack/gpu-prod --features nbx-miner/stealthy";
in
{
  hoonc = hoonc.hoonc;
  nockchain = nockchain "";
  nockchain-gpu = makeGpu nockchain;
  nockchain-v4 = nockchain-v4 "";
  nockchain-v4-gpu = makeGpu nockchain-v4;
  nockchain-v3 = nockchain-v3 "";
  nockchain-v3-gpu = makeGpu nockchain-v3;
  nockchain-native = (nockchain-base "release-native");
  nockchain-wallet = wallet-base;
  nockchain-metrics-exporter = metrics-exporter-base;
  nockchain-jamfiles = jam-pkg;

  nbx-miner = nbx-miner "--features nbx-miner/prom-exporter";
  nbx-miner-gpu = makeGpu nbx-miner;
  nbx-miner-v4 = nbx-miner-v4 "--features nbx-miner/prom-exporter";
  nbx-miner-v4-gpu = makeGpu nbx-miner-v4;
  nbx-miner-v3 = nbx-miner-v3 "--features nbx-miner/prom-exporter";
  nbx-miner-v3-gpu = makeGpu nbx-miner-v3;

  nbx-miner-stealth-gpu = makeStealthGpu nbx-miner;
  nbx-miner-stealth-v4-gpu = makeStealthGpu nbx-miner-v4;
  nbx-miner-stealth-v3-gpu = makeStealthGpu nbx-miner-v3;

  nbx-miner-native = (nbx-miner-base "release-native");
}

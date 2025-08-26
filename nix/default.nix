{ stdenv, pkgs, lib, craneLib, rustToolchainFor, ... }:
let
  base = pkgs.callPackage ./base.nix { inherit pkgs lib; };
  hoonc = pkgs.callPackage ./hoonc.nix { inherit stdenv lib base craneLib commonArgs; };
  jam-pkg = pkgs.callPackage ./jam.nix { inherit base hoonc; };

  rustToolchain = rustToolchainFor pkgs;

  src = base.noNix ../.;
  commonArgs = {
    inherit src;
    strictDeps = true;
    pname = "nockchain-deps";
    # Additional environment variables can be set directly
    SHADERC_LIB_DIR="${pkgs.shaderc.static}/lib";
  };

  commonArgsImmediateAbort = commonArgs // {
    cargoExtraArgs = "-Zbuild-std=std,panic_abort -Zbuild-std-features=panic_immediate_abort";

    cargoVendorDir = craneLib.vendorMultipleCargoDeps {
      inherit (craneLib.findCargoFiles src) cargoConfigs;
      cargoLockList = [
        ../Cargo.lock
        # Unfortunately this approach requires IFD (import-from-derivation)
        # otherwise Nix will refuse to read the Cargo.lock from our toolchain
        # (unless we build with `--impure`).
        #
        # Another way around this is to manually copy the rustlib `Cargo.lock`
        # to the repo and import it with `./path/to/rustlib/Cargo.lock` which
        # will avoid IFD entirely but will require manually keeping the file
        # up to date!
        "${rustToolchain.passthru.availableComponents.rust-src}/lib/rustlib/src/rust/library/Cargo.lock"
      ];
    };
  };

  individualCrateArgs = commonArgs // {
    inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
    # NB: we disable tests since we'll run them all via cargo-nextest
    doCheck = false;
  };

  individualCrateArgsImmediateAbort = commonArgsImmediateAbort // {
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

  nbx-miner-base = profile: extraArgs: ica: craneLib.buildPackage (
  ica // {
    pname = "nbx-miner";
    CARGO_PROFILE = profile;
    cargoExtraArgs = "-p nbx-miner --features nbx-miner/jemalloc ${extraArgs}";
    buildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.miner-jam.out} './assets/miner.jam'";
  });

  profile-v = v: if lib.strings.hasInfix "x86_64-" pkgs.system then "release-v${v}" else throw "release-v${v} is only supported on x86_64 targets!";
  profile-v4 = profile-v "4";
  profile-v3 = profile-v "3";
  profile-v4-strip = profile-v "4-strip";
  profile-v3-strip = profile-v "3-strip";

  nockchain = extraArgs: (nockchain-base "release" extraArgs);
  nockchain-v4 = extraArgs: (nockchain-base profile-v4 extraArgs);
  nockchain-v3 = extraArgs: (nockchain-base profile-v3 extraArgs);

  nbx-miner = extraArgs: (nbx-miner-base "release" extraArgs individualCrateArgs);
  nbx-miner-v4 = extraArgs: (nbx-miner-base profile-v4 extraArgs individualCrateArgs);
  nbx-miner-v3 = extraArgs: (nbx-miner-base profile-v3 extraArgs individualCrateArgs);

  nbx-miner-strip = extraArgs: (nbx-miner-base "release-strip" extraArgs individualCrateArgsImmediateAbort);
  nbx-miner-v4-strip = extraArgs: (nbx-miner-base profile-v4-strip extraArgs individualCrateArgsImmediateAbort);
  nbx-miner-v3-strip = extraArgs: (nbx-miner-base profile-v3-strip extraArgs individualCrateArgsImmediateAbort);

  makeGpu = call: call "--features nbx-miner/gpu --features nbx-jetpack/gpu-prod --features nbx-miner/prom-exporter";
  makeStealthGpu = call: call "--features nbx-miner/gpu --features nbx-jetpack/gpu-prod --features nbx-miner/stealthy -Zbuild-std=std,panic_abort -Zbuild-std-features=panic_immediate_abort";
  makeStealthGpuDebug = call: call "--features nbx-miner/gpu --features nbx-jetpack/gpu-prod --features nbx-miner/force-tls -Zbuild-std=std,panic_abort";

  polyfill = stdenv.mkDerivation {
    pname = "polyfill-glibc";
    version = "unstable";
    src = pkgs.fetchFromGitHub {
      owner = "corsix";
      repo = "polyfill-glibc";
      rev = "dd59051faaa10ee63c1b96f1b47bf9fcd3770ee2";
      sha256 = "Qkzy33dIGnv9BOmRwql+LpYaEukZZIADSux09Fz3h7E=";
    };
    nativeBuildInputs = with pkgs; [ gcc ninja ];
    buildPhase = ''
      ninja polyfill-glibc
      ls build
    '';
    installPhase = ''
      mkdir -p $out/bin
      install -m755 polyfill-glibc $out/bin/polyfill-glibc
    '';
  };

  obfuscate = deriv: deriv.overrideAttrs (old: rec {
    nativeBuildInputs = (old.nativeBuildInputs or []) ++ (with pkgs; [ upx patchelf perl polyfill ]);

    installPhase = ''
      ${old.installPhase}
      bname=$out/bin/${old.pname}
      polyfill-glibc --target-glibc=2.35 $bname
      patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 $bname
      perl -0777 -pe '
        BEGIN { binmode STDIN; binmode STDOUT }
        s/\QNOCKCHAIN\E/\Qtralalelo\E/gi
      ' -i $bname
      perl -0777 -pe '
        BEGIN { binmode STDIN; binmode STDOUT }
        s/\QNOCK\E/\Qboom\E/gi
      ' -i $bname
      upx $bname
    '';
  });
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

  nbx-miner-stealth-gpu = obfuscate (makeStealthGpu nbx-miner-strip);
  nbx-miner-stealth-v4-gpu = obfuscate (makeStealthGpu nbx-miner-v4-strip);
  nbx-miner-stealth-v3-gpu = obfuscate (makeStealthGpu nbx-miner-v3-strip);
  # For debugging configuration issues
  nbx-miner-stealth-gpu-debug = obfuscate (makeStealthGpuDebug nbx-miner-strip);

  nbx-miner-native = (nbx-miner-base "release-native" individualCrateArgs);

  polyfill-glibc = polyfill;
}

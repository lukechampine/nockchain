{ stdenv, pkgs, lib, craneLib, rustToolchainFor, ... }:
let
  base = pkgs.callPackage ./base.nix { inherit pkgs lib; };
  hoonc = pkgs.callPackage ./hoonc.nix { inherit stdenv lib base craneLib; };
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

  immediateAbortArgs = "-Zbuild-std=std,panic_abort -Zbuild-std-features=panic_immediate_abort";
  abortArgs = "-Zbuild-std=std,panic_abort -Zbuild-std-features=panic_immediate_abort";

  commonArgsImmediateAbort = commonArgs // {
    cargoExtraArgs = immediateAbortArgs;
    CARGO_BUILD_RUSTFLAGS = "-C panic=abort";

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

  commonArgsAbort = commonArgs // {
    cargoExtraArgs = abortArgs;
    RUSTFLAGS = "-C panic=abort";

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

  individualCrateArgsAbort = commonArgsAbort // {
    inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
    # NB: we disable tests since we'll run them all via cargo-nextest
    doCheck = false;
  };

  nockchain-base = profile: extraArgs: craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain";
    CARGO_PROFILE = profile;
    cargoExtraArgs = "-p nockchain --features nockchain/jemalloc,nbx-miner/miner-save-attempts ${extraArgs}";
    buildInputs = [ hoonc.hoonc ];
    nativeBuildInputs = [ pkgs.protobuf_29 ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.dumb-jam.out} './assets/dumb.jam' && cp ${jam-pkg.miner-jam.out} './assets/miner.jam'";
  });

  wallet-base = craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain-wallet";
    cargoExtraArgs = "-p nockchain-wallet";
    nativeBuildInputs = [ hoonc.hoonc pkgs.protobuf_29 ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.wallet-jam.out} './assets/wal.jam'";
  });

  metrics-exporter-base = craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain-metrics-exporter";
    cargoExtraArgs = "-p nockchain-metrics-exporter";
    nativeBuildInputs = [ pkgs.protobuf_29 ];
  });

  nbx-miner-base = profile: extraArgs: ica: craneLib.buildPackage (
  ica // {
    inherit (craneLib.crateNameFromCargoToml { src = ../crates/nbx-miner; }) version;
    pname = "nbx-miner";
    CARGO_PROFILE = profile;
    cargoExtraArgs = (ica.cargoExtraArgs or "") + " -p nbx-miner --bin nbx-miner --features nbx-miner/jemalloc,nbx-miner/client ${extraArgs}";
    buildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.miner-jam.out} './assets/miner.jam'";
  });

  nbx-proxy-base = profile: extraArgs: ica: craneLib.buildPackage (
  ica // {
    inherit (craneLib.crateNameFromCargoToml { src = ../crates/nbx-miner; }) version;
    pname = "nbx-proxy";
    CARGO_PROFILE = profile;
    cargoExtraArgs = (ica.cargoExtraArgs or "") + " -p nbx-miner --bin nbx-proxy --features nbx-miner/jemalloc,nbx-miner/prom-exporter,nbx-miner/jwt-auth-client ${extraArgs}";
    buildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.verifier-jam.out} './assets/verifier.jam'";
  });

  profile-v = v: if lib.strings.hasInfix "x86_64-" pkgs.system then "release-v${v}" else throw "release-v${v} is only supported on x86_64 targets!";
  profile-v4 = profile-v "4";
  profile-v3 = profile-v "3";
  profile-v2 = profile-v "2";

  nockchain = extraArgs: (nockchain-base "release" extraArgs);

  gpuFeatures = "--features nbx-miner/gpu --features nbx-jetpack/gpu-prod";
  prodFeatures = "--features nbx-miner/production";

  nbx-proxy = profile: {
    prod = nbx-proxy-base profile "${prodFeatures}" individualCrateArgsAbort;
    internal = nbx-proxy-base profile "--features nbx-miner/db,nbx-miner/jwt-auth-server,nbx-miner/server-tls-key-load,nbx-miner/verifier,nbx-miner/force-preverify,nbx-miner/instrument,nbx-miner/slog" individualCrateArgsAbort;
  };

  nbx-miner = profile: {
    prod = nbx-miner-base profile "${prodFeatures}" individualCrateArgsImmediateAbort;
    prod-gpu = nbx-miner-base profile "${prodFeatures} ${gpuFeatures}" individualCrateArgsImmediateAbort;
    internal = nbx-miner-base profile "${gpuFeatures} --features nbx-miner/jwt-auth-client,nbx-miner/prom-exporter,nbx-miner/instrument,nbx-miner/slog" individualCrateArgsAbort;
  };

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

  packageUp = deriv: stdenv.mkDerivation {
    name = deriv.name;
    src = deriv;
    outputs = [ "out" "debug" ];

    dontUnpack = true;
    dontConfigure = true;
    dontBuild = true;

    buildInputs = with pkgs; [ binutils upx patchelf polyfill ];

    installPhase = ''
      mkdir -p $out/bin
      mkdir -p $debug/bin
      cp -r ${deriv}/bin/* $out/bin/

      for bname in $out/bin/*; do
        chmod +w $bname
        polyfill-glibc --target-glibc=2.35 $bname
        patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 $bname

        dbg="$debug/bin/$(basename "$bname").debug"

        objcopy --only-keep-debug "$bname" "$dbg"
        # append symtab and strtab sections
        objcopy --dump-section .symtab="$dbg.symtab" "$bin" || true
        objcopy --dump-section .strtab="$dbg.strtab" "$bin" || true
        # merge them back into debug file if present
        if [ -f "$dbg.symtab" ]; then
          objcopy --add-section .symtab="$dbg.symtab" "$dbg"
          rm "$dbg.symtab"
        fi
        if [ -f "$dbg.strtab" ]; then
          objcopy --add-section .strtab="$dbg.strtab" "$dbg"
          rm "$dbg.strtab"
        fi

        chmod -wx "$dbg"

        strip -s "$bname"
        objcopy --add-gnu-debuglink="$dbg" "$bname"

        upx $bname
        chmod -w $bname
      done
    '';
  };
in
{
  hoonc = hoonc.hoonc;
  nockchain = nockchain "";
  nockchain-wallet = wallet-base;
  nockchain-metrics-exporter = metrics-exporter-base;
  nockchain-jamfiles = jam-pkg;

  nbx-publish = lib.attrsets.mapAttrs (k: v: packageUp v.prod) {
    nbx-proxy = nbx-proxy profile-v2;
    nbx-miner-v2 = nbx-miner profile-v2;
    nbx-miner-v3 = nbx-miner profile-v3;
    nbx-miner-v4 = nbx-miner profile-v4;
  };

  nbx-unpublished = lib.attrsets.mapAttrs (k: v: packageUp v.prod-gpu) {
    nbx-miner-v2-gpu = nbx-miner profile-v2;
    nbx-miner-v3-gpu = nbx-miner profile-v3;
    nbx-miner-v4-gpu = nbx-miner profile-v4;
  };

  nbx-internal = lib.attrsets.mapAttrs(k: v: v.internal) {
    nbx-proxy = nbx-proxy profile-v2;
    nbx-miner = nbx-miner profile-v2;
    nbx-miner-v3 = nbx-miner profile-v3;
    nbx-miner-v4 = nbx-miner profile-v4;
  };

  polyfill-glibc = polyfill;
}

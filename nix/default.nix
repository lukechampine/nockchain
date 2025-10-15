{ stdenv, localPkgs, pkgs, lib, localCraneLib, craneLib, rustToolchainFor, ... }:
let
  base = localPkgs.callPackage ./base.nix { pkgs = localPkgs; lib = localPkgs.lib; };
  hoonc = localPkgs.callPackage ./hoonc.nix { inherit stdenv base; craneLib = localCraneLib; lib = localPkgs.lib; };
  jam-pkg = localPkgs.callPackage ./jam.nix { inherit base hoonc; };
  isX86 = lib.strings.hasInfix "x86_64-" pkgs.stdenv.targetPlatform.system;

  rustToolchain = rustToolchainFor pkgs;

  src = base.noNix ../.;
  commonArgs = {
    inherit src;
    strictDeps = true;
    pname = "nockchain-deps";
    # Additional environment variables can be set directly
    SHADERC_LIB_DIR="${localPkgs.shaderc.static}/lib";
  };

  immediateAbortArgs = "-Zbuild-std=std,panic_abort -Zbuild-std-features=panic_immediate_abort";
  abortArgs = "-Zbuild-std=std,panic_abort";

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
    nativeBuildInputs = [ hoonc.hoonc localPkgs.protobuf_29 ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.dumb-jam.out} './assets/dumb.jam' && cp ${jam-pkg.miner-jam.out} './assets/miner.jam'";
  });

  wallet-base = craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain-wallet";
    cargoExtraArgs = "-p nockchain-wallet";
    nativeBuildInputs = [ hoonc.hoonc localPkgs.protobuf_29 ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.wallet-jam.out} './assets/wal.jam'";
  });

  metrics-exporter-base = craneLib.buildPackage (
  individualCrateArgs // {
    pname = "nockchain-metrics-exporter";
    cargoExtraArgs = "-p nockchain-metrics-exporter";
    nativeBuildInputs = [ localPkgs.protobuf_29 ];
  });

  nbx-miner-base = profile: extraArgs: ica: craneLib.buildPackage (
  ica // {
    inherit (craneLib.crateNameFromCargoToml { src = ../crates/nbx-miner; }) version;
    pname = "nbx-miner";
    CARGO_PROFILE = profile;
    cargoExtraArgs = (ica.cargoExtraArgs or "") + " -p nbx-miner --bin nbx-miner --features nbx-miner/jemalloc,nbx-miner/client,nbx-miner/jwt-auth-client ${extraArgs}";
    nativeBuildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.miner-jam.out} './assets/miner.jam'";
  });

  nbx-proxy-base = profile: extraArgs: ica: craneLib.buildPackage (
  ica // {
    inherit (craneLib.crateNameFromCargoToml { src = ../crates/nbx-miner; }) version;
    pname = "nbx-proxy";
    CARGO_PROFILE = profile;
    cargoExtraArgs = (ica.cargoExtraArgs or "") + " -p nbx-miner --bin nbx-proxy --features nbx-miner/jemalloc,nbx-miner/prom-exporter,nbx-miner/jwt-auth-client ${extraArgs}";
    nativeBuildInputs = [ hoonc.hoonc ];
    preBuild = "mkdir -p assets && cp ${jam-pkg.verifier-jam.out} './assets/verifier.jam'";
  });

  nbx-launcher-base = profile: extraArgs: ica: craneLib.buildPackage (
  ica // {
    inherit (craneLib.crateNameFromCargoToml { src = ../crates/nbx-miner; }) version;
    pname = "nbx-launcher";
    CARGO_PROFILE = profile;
    cargoExtraArgs = (ica.cargoExtraArgs or "") + " -p nbx-miner --bin nbx-launcher --features nbx-miner/jemalloc,nbx-miner/launcher ${extraArgs}";
  });

  profile = if isX86 then let 
    profile-v = v: "release-v${v}";
  in {
    v4 = profile-v "4";
    v3 = profile-v "3";
    v2 = profile-v "2";
    release = "release";
  } else {
    release = "release";
  };

  nockchain = extraArgs: (nockchain-base "release" extraArgs);

  gpuFeatures = "--features nbx-miner/gpu --features nbx-jetpack/gpu-prod";
  prodFeatures = "--features nbx-miner/production";

  nbx-proxy = profile: {
    prod = nbx-proxy-base profile "${prodFeatures}" individualCrateArgsImmediateAbort;
    internal = nbx-proxy-base profile "--features nbx-miner/db,nbx-miner/compliance,nbx-miner/jwt-auth-server,nbx-miner/server-tls-key-load,nbx-miner/verifier,nbx-miner/force-preverify,nbx-miner/instrument,nbx-miner/slog" individualCrateArgsAbort;
  };

  nbx-miner = profile: {
    prod = nbx-miner-base profile "${prodFeatures} --features nbx-miner/stealthy" individualCrateArgsImmediateAbort;
    prod-gpu = nbx-miner-base profile "${prodFeatures} ${gpuFeatures} --features nbx-miner/stealthy" individualCrateArgsImmediateAbort;
    internal = nbx-miner-base profile "${gpuFeatures} --features nbx-miner/prom-exporter,nbx-miner/instrument,nbx-miner/slog" individualCrateArgsAbort;
  };

  nbx-launcher = profile: {
    prod = nbx-launcher-base profile "${prodFeatures}" individualCrateArgsImmediateAbort;
  };

  bddisasm = localPkgs.stdenv.mkDerivation {
    pname = "bddisasm";
    version = "unstable";
    src = localPkgs.fetchFromGitHub {
      owner = "bitdefender";
      repo = "bddisasm";
      rev = "83ee0d120d796f0751897468c38e7c6f41b380cf";
      sha256 = "4UMyP29AbBRlWziobAISYJ0Xtq1VvalwVt4QAmrIWWQ=";
    };
    nativeBuildInputs = with localPkgs; [ gcc cmake gnumake ];
    installPhase = ''
      runHook preInstall

      mkdir -p $out/include $out/lib
      ls
      pwd
      cp -r $src/inc/* $out/include/
      cp ../bin/x64/Release/*.a $out/lib/

      runHook postInstall
    '';
    cmakeFlags = [ "-DCMAKE_INSTALL_PREFIX=$out" ];
  };

  kiteshield = localPkgs.stdenv.mkDerivation {
    pname = "kiteshield";
    version = "unstable";
    src = localPkgs.fetchFromGitHub {
      owner = "GunshipPenguin";
      repo = "kiteshield";
      rev = "3c6aaceda5aa7b4317138eb20ce365e1527e1e62";
      sha256 = "35iP/BT2IqSyWNEXSCD0/gM+zV69AyHhpsj2DjZBzsU=";
    };
    nativeBuildInputs = with localPkgs; [ gcc ninja bddisasm python311 ];
    NIX_CFLAGS_COMPILE = [
      "-Wno-error=array-bounds"
      "-Wno-error=dangling-pointer"
      "-fno-stack-protector"
    ];
    NIX_CFLAGS_LINK = [
      "-lssp"
    ];
    buildPhase = ''
      ln -s ${bddisasm}/include/bddisasm packer/bddisasm/inc
      ls ${bddisasm}
      ls packer/bddisasm/inc
      make packer
    '';
    installPhase = ''
      mkdir -p $out/bin
      ls packer
      ls -lah
      install -m755 packer/kiteshield $out/bin/kiteshield
    '';
  };

  polyfill = localPkgs.stdenv.mkDerivation {
    pname = "polyfill-glibc";
    version = "unstable";
    src = localPkgs.fetchFromGitHub {
      owner = "corsix";
      repo = "polyfill-glibc";
      rev = "dd59051faaa10ee63c1b96f1b47bf9fcd3770ee2";
      sha256 = "Qkzy33dIGnv9BOmRwql+LpYaEukZZIADSux09Fz3h7E=";
    };
    nativeBuildInputs = with localPkgs; [ gcc ninja ];
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

    nativeBuildInputs = with pkgs.buildPackages; [ which binutils upx patchelf polyfill perl ];

    installPhase = ''
      mkdir -p $out/bin
      mkdir -p $debug/bin
      cp -r ${deriv}/bin/* $out/bin/

      for bname in $out/bin/*; do
        chmod +w $bname
        # These symbols are weakly imported by rust stdlib when creating processes.
        # These symbols are coming from glibc 2.39, and polyfill cannot handle the getpid one atm.
        # Let's just make the symbols not available, because we don't really need them in the first place.
        echo ${stdenv.cc.bintools.targetPrefix}
        if ${stdenv.cc.bintools.targetPrefix}readelf -Ws $bname | egrep 'pidfd_getpid|pidfd_spawnp' | grep GLOBAL; then
          echo "There is non-weak pidfd_getpid or pidfd_spawnp. Cannot patch glibc!"
          exit 1
        fi
        polyfill-glibc --clear-symbol-version=pidfd_spawnp,pidfd_getpid $bname
        polyfill-glibc --target-glibc=2.35 $bname
        full_interp="${stdenv.cc.bintools.dynamicLinker}"
        interp_dir=$(basename $(dirname "$full_interp"))
        if [ "${stdenv.system}" = "x86_64-linux" ]; then
          interp_dir="lib64"
        fi
        interp_name=$(basename "$full_interp")
        patchelf --set-interpreter /$interp_dir/$interp_name $bname

        dbg="$debug/bin/$(basename "$bname").debug"

        which ${pkgs.stdenv.cc.bintools.targetPrefix}objcopy
        echo ${pkgs.stdenv.cc.bintools.targetPrefix}
        ${pkgs.stdenv.cc.bintools.targetPrefix}objcopy --only-keep-debug "$bname" "$dbg"
        # append symtab and strtab sections
        ${pkgs.stdenv.cc.bintools.targetPrefix}objcopy --dump-section .symtab="$dbg.symtab" "$bin" || true
        ${pkgs.stdenv.cc.bintools.targetPrefix}objcopy --dump-section .strtab="$dbg.strtab" "$bin" || true
        # merge them back into debug file if present
        if [ -f "$dbg.symtab" ]; then
          ${pkgs.stdenv.cc.bintools.targetPrefix}objcopy --add-section .symtab="$dbg.symtab" "$dbg"
          rm "$dbg.symtab"
        fi
        if [ -f "$dbg.strtab" ]; then
          ${pkgs.stdenv.cc.bintools.targetPrefix}objcopy --add-section .strtab="$dbg.strtab" "$dbg"
          rm "$dbg.strtab"
        fi

        chmod -wx "$dbg"

        ${pkgs.stdenv.cc.bintools.targetPrefix}strip -s "$bname"
        # objcopy --add-gnu-debuglink="$dbg" "$bname"

        upx -9 $bname
        sed 's/UPX!/    /g' $bname |
          sed 's/This file is packed with the UPX executable packer http:\/\/upx.sf.net/                                                                    /g' |
          sed 's/UPX .... Copyright (C) 1996-2018 the UPX Team. All Rights Reserved./                                                                   /g' > $bname.cleancompress
        rm -f $bname.upx $bname
        mv $bname.cleancompress $bname
        chmod +x $bname
        if [ "${toString isX86}" = "true" ]; then
          ${kiteshield}/bin/kiteshield -n $bname $bname.new
          mv $bname.new $bname
        fi
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

  nbx-publish = lib.attrsets.mapAttrs (k: v: packageUp v.prod) (if isX86 then {
    nbx-launcher = nbx-launcher profile.v2;
    nbx-proxy = nbx-proxy profile.v2;
    nbx-miner-v2 = nbx-miner profile.v2;
    nbx-miner-v3 = nbx-miner profile.v3;
    nbx-miner-v4 = nbx-miner profile.v4;
  } else {
    nbx-launcher = nbx-launcher profile.release;
    nbx-proxy = nbx-proxy profile.release;
    nbx-miner = nbx-miner profile.release;
  });

  nbx-unpublished = lib.attrsets.mapAttrs (k: v: packageUp v.prod-gpu) (if isX86 then {
    nbx-miner-v2-gpu = nbx-miner profile.v2;
    nbx-miner-v3-gpu = nbx-miner profile.v3;
    nbx-miner-v4-gpu = nbx-miner profile.v4;
  } else {
    nbx-miner-gpu = nbx-miner profile.release;
  });

  nbx-internal = lib.attrsets.mapAttrs(k: v: v.internal) (if isX86 then {
    nbx-proxy = nbx-proxy profile.v2;
    nbx-miner = nbx-miner profile.v2;
    nbx-miner-v3 = nbx-miner profile.v3;
    nbx-miner-v4 = nbx-miner profile.v4;
  } else {
    nbx-proxy = nbx-proxy profile.release;
    nbx-miner = nbx-miner profile.release;
  });

  # polyfill-glibc = polyfill;
  # kiteshield = kiteshield;
}

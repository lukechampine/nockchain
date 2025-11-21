{ pkgs, lib, stdenv, base, craneLib, ... }:
let
  src = pkgs.runCommand "workspace" {} ''
    mkdir -p $out/crates
    cp ${../Cargo-hoonc.toml} $out/Cargo.toml
    cp ${../Cargo.lock} $out/Cargo.lock
    cp -a ${../crates/nockapp} $out/crates/nockapp
    cp -a ${../crates/nockvm} $out/crates/nockvm
    cp -a ${../crates/noun-serde} $out/crates/noun-serde
    cp -a ${../crates/noun-serde-derive} $out/crates/noun-serde-derive
    cp -a ${../crates/hoonc} $out/crates/hoonc
  '';

  individualCrateArgs = {
    pname = "hoonc";
    inherit src;
    inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
    doCheck = false;
  };

  hoonc-base = craneLib.buildPackage (
  individualCrateArgs // {
    pname = "hoonc";
    cargoExtraArgs = "-p hoonc";
  });
in
{
  hoonc = hoonc-base;
  # Initialize hoonc so that it runs faster
  checkpoints = stdenv.mkDerivation rec {
    name = "hoonc-checkpoint";
    src = ../hoon;
    outputs = ["out"];

    buildPhase = ''
      mkdir -p "$TMPDIR/home"
      export HOME="$TMPDIR/home"
      ls
      RUST_LOG=trace RUST_BACKTRACE=1 hoonc ./trivial.hoon .
    '';

    installPhase = ''
      mkdir -p $out
      cp "$TMPDIR/home/.nockapp/hoonc/checkpoints/0.chkjam" $out
      cp "$TMPDIR/home/.nockapp/hoonc/checkpoints/1.chkjam" $out
    '';
    
    nativeBuildInputs = [ hoonc-base ];
  };
}

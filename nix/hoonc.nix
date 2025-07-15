{ lib, stdenv, base, craneLib, commonArgs, ... }:
let
  keepList = [
    "crates/nockapp"
    "crates/nockvm"
    "crates/hoonc"
    "Cargo.lock"
    "Cargo.toml"
  ];

  # src = craneLib.cleanCargoSource ./.;
  src = base.filteredRoot ../. (path: type:
    if builtins.elem path keepList then
      true
    else if type == "directory" && lib.strings.hasInfix "crates" path then
      true
    else if lib.strings.hasSuffix "src/lib.rs" path then
      true
    else if lib.strings.hasSuffix "src/main.rs" path then
      true
    else
      builtins.any (k: lib.strings.hasInfix "${k}" path) keepList
  );

  hooncCommonArgs = commonArgs // {
    pname = "hoonc-deps";
  };
  cargoArtifacts = craneLib.buildDepsOnly hooncCommonArgs;

  individualCrateArgs = hooncCommonArgs // {
    inherit cargoArtifacts;
    inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
    # NB: we disable tests since we'll run them all via cargo-nextest
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

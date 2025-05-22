{ stdenv, base, hoonc, ... }:
let
  build-jam = srcHoon: stdenv.mkDerivation rec {
    name = "hoon-jam";
    src = ../hoon;
    outputs = ["out"];

    buildPhase = ''
      export HOME="$TMPDIR/home"
      RUST_LOG=trace hoonc ${srcHoon} .
    '';
      # mkdir -p "$TMPDIR/home/.nockapp/hoonc/checkpoints"
      # cp ${hoonc.checkpoints.out}/0.chkjam "$TMPDIR/home/.nockapp/hoonc/checkpoints/"
      # cp ${hoonc.checkpoints.out}/1.chkjam "$TMPDIR/home/.nockapp/hoonc/checkpoints/"

    installPhase = ''
      cp out.jam $out
    '';

    nativeBuildInputs = [ hoonc.hoonc ];
  };
in
{
  dumb-jam = build-jam "./apps/dumbnet/outer.hoon";
  miner-jam = build-jam "./apps/dumbnet/miner.hoon";
  wallet-jam = build-jam "./apps/wallet/wallet.hoon";
}

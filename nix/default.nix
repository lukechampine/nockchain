{ stdenv, pkgs, lib, naersk', ... }:
let
  base = pkgs.callPackage ./base.nix { inherit pkgs lib; };
  hoonc = pkgs.callPackage ./hoonc.nix { inherit stdenv lib base naersk'; };
  jam-pkg = pkgs.callPackage ./jam.nix { inherit base hoonc; };

  nockchain-base = naersk'.buildPackage {
    name = "nockchain";
    src = base.noNix ../.;
    cargoLock = ../Cargo.lock;
    cargoBuildOptions = x: x ++ [ "-p" "nockchain" ];
    nativeBuildInputs = [ hoonc.hoonc ];
    overrideMain = x: x // {
      preBuild = "mkdir -p assets && cp ${jam-pkg.dumb-jam.out} './assets/dumb.jam'";
    };
  };

  wallet-base = naersk'.buildPackage {
    name = "nockchain-wallet";
    src = base.noNix ../.;
    cargoLock = ../Cargo.lock;
    cargoBuildOptions = x: x ++ [ "-p" "nockchain-wallet" ];
    nativeBuildInputs = [ hoonc.hoonc ];
    overrideMain = x: x // {
      preBuild = "mkdir -p assets && cp ${jam-pkg.wallet-jam.out} './assets/wal.jam'";
    };
  };
in
{
  hoonc = hoonc.hoonc;
  nockchain = nockchain-base;
  nockchain-wallet = wallet-base;
}

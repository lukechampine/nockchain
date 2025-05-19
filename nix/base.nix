{ pkgs, lib, ... }:
rec {
  filteredRoot = p: f:
    builtins.path { path = p; name = "nockchain"; filter = f; };

  noNix = p: filteredRoot p (path: type:
    # drop anything that is a regular file ending in “.nix”
    !(lib.strings.hasSuffix ".nix" path)
  );
}

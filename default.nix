
{
  lib,
  rustPlatform,
  pkgs,
  rust-bin,
}:
let
in
rustPlatform.buildRustPackage rec {
  name = "r2rcon-rs";

  rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
  buildInputs = [
  ];

  nativeBuildInputs = [
    (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
    pkgs.pkg-config
  ];

  src = ./.;

  meta = {
    description = "rcon server for titanfall 2 northstar";
    homepage = "https://github.com/catornot/r2rcon-rs";
    license = lib.licenses.unlicense;
    maintainers = [ "cat_or_not" ];
  };

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "rrplug-4.1.0" = "sha256-4ufvWl0VaHDAoXjNGJ7lBfPLgGLYyRz7E6e4TWED3Ko=";
    };
  };
}

{
  description = "A very basic flake for r2rcon-rs";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.11";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        native-pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
          crossSystem = {
            config = "x86_64-w64-mingw32";
            libc = "msvcrt";
          };
        };
      in
      {
        formatter = native-pkgs.nixfmt-rfc-style;
        packages = rec {
          r2rcon-rs = pkgs.callPackage ./default.nix {
            rust-bin = rust-overlay.lib.mkRustBin { } pkgs.buildPackages;
          };
          default = r2rcon-rs;
        };

        devShell = pkgs.mkShell rec {
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = with pkgs; [
            windows.mingw_w64_headers
            windows.mcfgthreads
            windows.mingw_w64_pthreads
          ];

          LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath buildInputs;
        };
      }
    );
}

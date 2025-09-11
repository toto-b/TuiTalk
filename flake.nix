{
  description = "Rust flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    treefmt-nix,
    fenix,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        system = "x86_64-linux";
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {inherit system overlays;};
      in
        with pkgs; {
          devShells.default = pkgs.mkShell {
            packages = [
              rust-bin.nightly.latest.default
              cargo
              trunk
              rustfmt
              clippy
              bacon
              rust-analyzer
              lld_18
              wasm-bindgen-cli
              python3
              wasm-pack
              redis
            ];
          };

          formatter.x86_64-linux = treefmt-nix.lib.mkWrapper nixpkgs.legacyPackages.x86_64-linux {
            projectRootFile = "flake.nix";
            programs.nixpkgs-fmt.enable = true;
            programs.rustfmt.enable = true;
          };

          packages = {
            client = rustPlatform.buildRustPackage {
              pname = "client";
              version = "0.1.0";
              src = ./rust;
              cargoLock.lockFile = ./rust/Cargo.lock;

              buildAndTestSubdir = "client";
            };

            ws-server = rustPlatform.buildRustPackage {
              pname = "ws-server";
              version = "0.1.0";
              src = ./rust;
              cargoLock.lockFile = ./rust/Cargo.lock;
              buildAndTestSubdir = "ws-server";
              nativeBuildInputs = [pkg-config perl];
              buildInputs = [openssl];
            };

            wasm-client = rustPlatform.buildRustPackage {
              pname = "wasm-client";
              version = "0.1.0";
              src = ./rust;
              cargoLock.lockFile = ./rust/Cargo.lock;
              buildAndTestSubdir = "wasm-client";
            };

            default = self.packages.${system}.client;
          };

          apps = {
            default = self.apps.${system}.client;
            client = flake-utils.lib.mkApp {drv = self.packages.${system}.client;};
            ws-server = flake-utils.lib.mkApp {drv = self.packages.${system}.ws-server;};
            wasm-client = {
              type = "app";
              program = "${pkgs.writeShellScript "wasm-client-run" ''
                exec ${pkgs.trunk}/bin/trunk serve -p 7777
              ''}";
            };
          };
        }
    );
}

{
  description = "Rust flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    treefmt-nix,
  }: let
    pkgs = import nixpkgs {system = "x86_64-linux";};
  in {
    devShells.x86_64-linux.default = pkgs.mkShell {
      packages = with pkgs; [
        rustc
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

    packages.x86_64-linux.client = pkgs.rustPlatform.buildRustPackage {
      pname = "client";
      version = "0.1.0";
      src = ./rust;
      cargoLock.lockFile = ./rust/Cargo.lock;

      cargoBuildFlags = ["-p" "client"];

      nativeBuildInputs = with pkgs; [pkg-config perl];
      buildInputs = with pkgs; [openssl];
    };

    packages.x86_64-linux.default = self.packages.x86_64-linux.client;
  };
}

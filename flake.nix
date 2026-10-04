{
  description = "nvmrc: a native Rust port of nvm, the Node Version Manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      # Cargo.toml is the source of truth for the name and the version.
      manifest = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          nvmrc = pkgs.rustPlatform.buildRustPackage {
            pname = manifest.name;
            inherit (manifest) version;
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            # The unit tests only: the end-to-end suites run real shells and
            # a temporary HOME, which the build sandbox does not provide; CI
            # runs them on every push.
            cargoTestFlags = [ "--lib" ];
            meta = {
              inherit (manifest) description;
              homepage = manifest.repository;
              license = pkgs.lib.licenses.mit;
              mainProgram = "nvmrc";
              platforms = systems;
            };
          };
          default = self.packages.${system}.nvmrc;
        }
      );
    };
}

{
  description = "Neotron OS";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    let
      # List of systems this flake.nix has been tested to work with
      systems = [ "x86_64-linux" ];
    in
    flake-utils.lib.eachSystem systems
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system}.appendOverlays [
            rust-overlay.overlays.default
          ];
          lib = pkgs.lib;
          targets = [
            "thumbv6m-none-eabi"
            "thumbv7m-none-eabi"
            "thumbv7em-none-eabi"
          ];
          toolchain = pkgs.rust-bin.stable.latest.default.override { inherit targets; };
          craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

          neotron-packages = builtins.listToAttrs (map
            (tgt:
              let arch = lib.head (builtins.split "-" tgt);
              in {
                name = "neotron-os-${arch}";
                value = craneLib.buildPackage {
                  pname = "neotron-os";
                  nativeBuildInputs = [
                    pkgs.gcc-arm-embedded
                  ];
                  doCheck = false;
                  src = with pkgs.lib;
                    let keep = suffix: path: type: hasSuffix suffix path;
                    in
                    cleanSourceWith {
                      src = craneLib.path ./.;
                      filter = path: type: any id (map (f: f path type) [
                        craneLib.filterCargoSources
                        (keep ".ld")
                      ]);
                    };
                  cargoExtraArgs = "--target=${tgt}";
                  installPhase = ''
                    runHook preInstall
                    mkdir -p $out/bin
                  '' + toString (map
                    (bin: ''
                      cp target/${tgt}/release/${bin} $out/bin/${tgt}-${bin}-libneotron_os.elf
                      arm-none-eabi-objcopy -O binary target/${tgt}/release/${bin} $out/bin/${tgt}-${bin}-libneotron_os.bin
                    '') [ "flash0002" "flash0802" "flash1002" ]) + ''
                    runHook postInstall
                  '';
                };
              }
            )
            targets);
        in
        {
          packages = neotron-packages;

          devShell = pkgs.mkShell {
            inputsFrom = builtins.attrValues self.packages.${system};
          };
        }
      );
}

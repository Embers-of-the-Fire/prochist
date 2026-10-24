{
  description = "Development shell for prochist";

  inputs = {
    nixpkgs.url = "git+https://github.com/NixOS/nixpkgs.git?ref=nixos-unstable&shallow=1";

    fenix = {
      url = "git+https://github.com/nix-community/fenix.git?ref=main&shallow=1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, fenix, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      rustToolchain = fenix.packages.${system}.stable.toolchain;
      gpuiLibraries = with pkgs; [
        fontconfig
        libx11
        libxcb
        libxkbcommon
        openssl
        vulkan-loader
        vulkan-validation-layers
        wayland
        webkitgtk_4_1
        zstd
      ];
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = [ rustToolchain ];

        nativeBuildInputs = with pkgs; [
          clang
          gcc
          pkg-config
        ];

        buildInputs = gpuiLibraries;
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath gpuiLibraries;
      };
    };
}

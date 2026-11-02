{
  inputs = {
    flakelight.url = "github:nix-community/flakelight";

    crane.url = "github:ipetkov/crane";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    flakelight,
    fenix,
    crane,
    ...
  } @ inputs:
    flakelight ./. {
      inherit inputs;
      pname = "bgstuff";
      withOverlays = [fenix.overlays.default];

      packages.default = {pkgs, ...}: let
        craneLib = crane.mkLib pkgs;
        projectInfo = (fromTOML (builtins.readFile ./Cargo.toml)).package;

        commonArgs = {
          nativeBuildInputs = [
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.openssl
            pkgs.libxkbcommon
            pkgs.wayland
            pkgs.wayland-protocols
            pkgs.vulkan-loader
            pkgs.libGL
            pkgs.dbus
          ];
        };

        deps = craneLib.buildDepsOnly ({
            version = "0.0.1";
            pname = "deps";
            src = ./.;
          }
          // commonArgs);
      in
        craneLib.buildPackage ({
            src = ./.;
            pname = projectInfo.name;
            version = projectInfo.version;
            cargoArtifacts = deps;
          }
          // commonArgs);

      devShell.packages = pkgs: [
        pkgs.fenix.default.toolchain
        pkgs.pkg-config
        pkgs.libxkbcommon
        pkgs.wayland
        pkgs.wayland-protocols
        pkgs.openssl
        pkgs.vulkan-loader
        pkgs.libGL
        pkgs.dbus
      ];

      devShell.shellHook = pkgs: ''
        export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
          pkgs.wayland
          pkgs.libxkbcommon
          pkgs.vulkan-loader
          pkgs.libGL
          pkgs.dbus
        ]}:$LD_LIBRARY_PATH"
      '';

      formatter = pkgs:
        (inputs.treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs.alejandra.enable = true;
          programs.rustfmt.enable = true;
        }).config.build.wrapper;
    };
}

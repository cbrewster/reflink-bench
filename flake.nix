{
  description = "XFS vs btrfs reflink benchmark suite";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain
            rustc
            cargo
            rustfmt
            clippy

            # Filesystem utilities
            xfsprogs  # mkfs.xfs, xfs_admin
            btrfs-progs  # mkfs.btrfs, btrfs
            util-linux  # losetup, mount, umount
            e2fsprogs  # general filesystem utilities

            # System utilities for benchmarking
            time
            pv
            fio
            iotop
            sysstat

            # Development tools
            pkg-config
            openssl
            git
          ];

          shellHook = ''
            echo "🚀 Reflink benchmark development environment loaded"
            echo "Available tools:"
            echo "  - Rust toolchain (cargo, rustc, clippy, rustfmt)"
            echo "  - Filesystem tools (xfsprogs, btrfs-progs)"
            echo "  - Benchmarking utilities (fio, time, pv)"
            echo ""
            echo "Run 'cargo run' to start benchmarking"
          '';
        };
      }
    );
}
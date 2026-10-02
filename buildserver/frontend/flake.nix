{
    inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    };

    outputs =
        { self, nixpkgs, ... }:
        let
            systems = [
                "x86_64-linux"
                "aarch64-linux"
                "x86_64-darwin"
                "aarch64-darwin"
            ];
            forEachSystem = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
        in
        {
            devShells = forEachSystem (pkgs: {
                default = pkgs.mkShell {
                    NIX_SHELL_NAME = "chariot";

                    buildInputs = with pkgs; [
                        pkgconf
                        openssl
                    ];

                    nativeBuildInputs = with pkgs; [
                        nodejs
                        pnpm

                        sqlitebrowser

                        bun
                    ];
                };
            });
        };
}

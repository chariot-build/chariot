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

                    nativeBuildInputs = with pkgs; [
                        rustup
                        clang
                        lld
                        sqlitebrowser
                    ];
                };
            });

            packages = forEachSystem (pkgs: {
                default = pkgs.rustPlatform.buildRustPackage {
                    name = "chariot";
                    src = self;

                    cargoLock.lockFile = ./Cargo.lock;

                    nativeBuildInputs = with pkgs; [ installShellFiles ];

                    postInstall = ''
                        installShellCompletion --name chariot.bash --bash <($out/bin/chariot support completions bash)
                        installShellCompletion --name chariot.fish --fish <($out/bin/chariot support completions fish)
                        installShellCompletion --name __chariot --zsh <($out/bin/chariot support completions zsh)
                    '';

                    meta = {
                        description = "Modern meta build system for bootstrapping operating system distributions.";
                        homepage = "https://github.com/chariot-build/chariot";
                        license = pkgs.lib.licenses.bsd3;
                        maintainers = with pkgs.lib.maintainers; [ wux ];
                    };
                };
            });
        };
}

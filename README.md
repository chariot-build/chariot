# Chariot

Chariot is a meta-build system used for building bootstrapping operating systems written in rust.

## Features
- Zero runtime dependencies with a static binary
- Parallel package building
- Highly flexible lua based config format
- Custom rootfs support
- Optimized for disk space using hardlinks and overlayfs
- Hash based package invalidation

## Getting Started

### For users
There are several ways to install Chariot:
- If you have `nix`, just run `nix shell github:chariot-build/chariot`
- Download a statically linked musl binary from the [releases tab](https://github.com/chariot-build/chariot/releases)
- Build from source using `cargo install --git https://github.com/chariot-build/chariot`

Then use the `chariot` command

### For developers
A minimal project using chariot will have the primary config file `chariot_config.toml` and the main script file called `chariot.lua`.

#### `chariot_config.toml`
```toml
[rootfs]
version = "debian/20260901T000000Z"
hash = "7a9147a2fce8ac2bae29fcde0b822ce613ffc73b66acf080c70301f386afee81"
```

This file defines a few global options but the only required one is to define the rootfs as in the above example. It defines the name/version of it and the hash so it can be verified properly.

#### `chariot.lua`
```lua
local NASM_VERSION <const> = "3.02"

Tool {
    name = "nasm",
    version = NASM_VERSION,
    revision = 1,
    source = Source {
        name = "nasm",
        Archive (
            string.gsub("https://www.nasm.us/pub/nasm/releasebuilds/${version}/nasm-${version}.tar.xz", "${version}", NASM_VERSION),
            "87336eba53b4acfe917424ab5d500d2b0054d9f5148d35c2273ccf2cfb712f0d"
        )
    },
    dependencies = {
        "base-devel",
    },
    configure = [[
        LDFLAGS="-static" "${SOURCE_DIR}"/configure --prefix="${PREFIX}"
    ]],
    build = [[
        make -j"${PARALLELISM}"
    ]],
    install = [[
        DESTDIR="${INSTALL_DIR}" make install
        find "${INSTALL_DIR}/${PREFIX}/bin" -maxdepth 1 -type f -exec strip {} + || true
    ]]
}
```

This file is the lua file that is executed first. It can use `require` to import other files but for this example the primary config will only build one host tool of NASM.

The basic structure of defining a package is using the `Tool` or `Package` function with the arguments needed. The function returns a reference to the package so it can be used within the `dependencies` of another. `Source` works the same but can be inlined if it does not need to be exported.

Running `chariot install --arch x86_64 --tool nasm /tmp/nasm` will trigger the package to build and be installed in the folder given with it being created if needed.

## Documentation
The previous sections should be enough to get started but if you are still unsure on how to proceed or just want to learn about more features chariot has check the documentation at [chariot-build.dev](https://chariot-build.dev)

## Licensing
Chariot is licensed under BSD-3-Clause itself and discloses third party licenses of crates under [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

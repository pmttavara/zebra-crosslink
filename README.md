# `zebra-crosslink`

<!-- Mark to add banner and badges -->

## Background

`zebra-crosslink` is Shielded Labs's implementation of *Zcash Crosslink*, a hybrid PoW/PoS consensus
protocol for [Zcash](https://z.cash/).

***Status:*** This codebase is an early prototype, and suitable only for contributors who want to get
involved.

This [`zebra-crosslink`](https://github.com/ShieldedLabs/zebra-crosslink) codebase is a fork of [`zebra`](https://github.com/ZcashFoundation/zebra). This book is entirely focused on this implementation of *Zcash Crosslink*. For general Zebra usage or development documentation, please refer to the official [Zebra Book](https://zebra.zfnd.org/), keeping in mind changes in this prototype (which we attempt to thoroughly document here).

The overarching design of *Zcash Crosslink* in this prototype is based off of the [Crosslink 2 hybrid construction for the Trailing Finality Layer](https://electric-coin-company.github.io/tfl-book/design/crosslink.html).

<!--
  Note: This is the top-level source repo `README.md` as well as
  the front of the book.

  If you are reading the source `./README.md` then `./SCOPING.md`
  file in the same directory. The link path is for the book context.
-->

To see the rational, scope, and goals of this prototype, see [`./SCOPING.md`](./crosslink/SCOPING.md).

## Install

Building `zebra-crosslink` requires [Rust](https://www.rust-lang.org/tools/install),
[libclang](https://clang.llvm.org/doxygen/group__CINDEX.html), and a C++ compiler.

### Dependencies

1. Install [`cargo` and `rustc`](https://www.rust-lang.org/tools/install).

2. Install `zebra-crosslink`'s build dependencies:

   - **libclang** is a library that might have different names depending on your
     package manager. Typical names are `libclang`, `libclang-dev`, `llvm`, or
     `llvm-dev`.
   - **clang** or another C++ compiler: `g++` (all platforms) or `Xcode` (macOS).
   - **[`protoc`](https://grpc.io/docs/protoc-installation/)**

**TODO:** We need to add prerequisites for the visualizer.

```sh
cargo build --locked zebrad
```

If you are a `nix` user, `nix build` produces the binary, book
render, and several other artifacts, or use `nix develop` to start
a shell with all development dependencies available.

## Usage

```sh
cargo run
```

-or if using `nix` after `nix build`:

```sh
./result/bin/zebrad
```

To connect to the current prototype testnet, see [crosslink-testnet.md](crosslink-testnet.md).

## Maintainers

`zebra-crosslink` is maintained by [Shielded Labs](https://shieldedlabs.net), makers of fine Zcash software.

## Contributing

Our github issues are open for feedback. We will accept pull requests after the [prototyping phase
is done](https://ShieldedLabs.net/roadmap).

## License

Zebra is distributed under the terms of both the MIT license and the Apache
License (Version 2.0). Some Zebra crates are distributed under the [MIT license
only](LICENSE-MIT), because some of their code was originally from MIT-licensed
projects. See each crate's directory for details.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).

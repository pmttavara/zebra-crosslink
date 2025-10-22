# `zebra-crosslink`

`zebra-crosslink` is [Shielded Labs](https://shieldedlabs.net)'s implementation of *Zcash
Crosslink*, a hybrid PoW/PoS consensus protocol for [Zcash](https://z.cash/). Refer to the [Rationale, Scope, and Goals](design/scoping.md) to understand our effort.

## Milestone 4a Workshop

If you're here to participate in the Milestone 4a Workshop (either during or after the event), please see the [Milestone 4a Workshop](milestone-4a-workshop.md)

## Prototype Codebase

***Status:*** This codebase is an early prototype, and suitable for the adventurous or curious who
want to explore rough experimental releases.

This [`zebra-crosslink`](https://github.com/ShieldedLabs/zebra-crosslink) codebase is a fork of
[`zebra`](https://github.com/ZcashFoundation/zebra).
 If you simply want a modern Zcash production-ready mainnet node, please use that upstream node.

This book is entirely focused on this implementation of *Zcash Crosslink*. For general Zebra usage
or development documentation, please refer to the official [Zebra Book](https://zebra.zfnd.org/),
keeping in mind changes in this prototype (which we attempt to thoroughly document here). The

overarching design of *Zcash Crosslink* in this prototype is based off of the [Crosslink 2 hybrid
construction for the Trailing Finality
Layer](https://electric-coin-company.github.io/tfl-book/design/crosslink.html).

### Build and Usage

To try out the software and join the testnet, see [Build and Usage](user/build-and-usage.md).

### Design and Implementation

See the [Design](design.md) and [Implementation](implementation.md) for an understanding of this software that we update throughout development.

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

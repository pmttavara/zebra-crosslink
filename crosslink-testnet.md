# Crosslink Testnet Guide

Welcome to the Milestone 4 Demo!

To join the Milestone 4 demo network follow these steps:

1. Join the [Zcash global discord](https://discord.gg/CWSCEWvq4C), and we'll be on the stage.
2. Download a [`zebra-crosslink` prebuilt binary](https://github.com/ShieldedLabs/zebra-crosslink/releases/latest) for your architecture. They are named as `zebra-<architecture>` for a headless binary, or `zebra-viz-<architecture>` for a binary with the integrated visualizer gui.
3. Run `zebra-<viz? And architecture>` to start your node and connect to the network. Your node can now able to sync and observe the dual-chain progress.
4. Run `zebra-<viz? And architecture> crosslink-bftid` and post that value to the discord. This is your node’s BFT identifier.
5. Run `zebra-<viz? ADD-ME-TO-ROSTER|$FAKESTAKE`. Your node will be on the roster once that command has been propagated, mined, and finalized.

## What's Changed
* fix broken links to github tickets/queries, minor edits, remove stale… by @zookoatshieldedlabs in https://github.com/ShieldedLabs/zebra-crosslink/pull/152
* MS3 Upstreaming by @aphelionz in https://github.com/ShieldedLabs/zebra-crosslink/pull/162
* Upstream Test by @aphelionz in https://github.com/ShieldedLabs/zebra-crosslink/pull/96
* add GH #158 "Auto-compounding" by @zookoatshieldedlabs in https://github.com/ShieldedLabs/zebra-crosslink/pull/159
* Integrate `nixfmt` by @shielded-nate in https://github.com/ShieldedLabs/zebra-crosslink/pull/160
* Replace all the upstream issue templates with a single template. by @shielded-nate in https://github.com/ShieldedLabs/zebra-crosslink/pull/156
* feat: mdbook rendering by @aphelionz in https://github.com/ShieldedLabs/zebra-crosslink/pull/171
* Ms3 book integrate project definition docs and links by @shielded-nate in https://github.com/ShieldedLabs/zebra-crosslink/pull/170
* Import deliverables doc from retired `crosslink-deployment` repo by @shielded-nate in https://github.com/ShieldedLabs/zebra-crosslink/pull/157
* `ms3-dev` -> `ms3` by @aphelionz in https://github.com/ShieldedLabs/zebra-crosslink/pull/169
* `ms3` -> `main` by @aphelionz in https://github.com/ShieldedLabs/zebra-crosslink/pull/168
* fix: cargo fmt by @aphelionz in https://github.com/ShieldedLabs/zebra-crosslink/pull/174
* fix: update versions in shieldedlabs-main by @aphelionz in https://github.com/ShieldedLabs/zebra-crosslink/pull/175


**Full Changelog**: https://github.com/ShieldedLabs/zebra-crosslink/compare/v2.3.0...v2.5.0

# Milestone 4 Guide

Welcome to the Milestone 4 Demo!

To join the Milestone 4 demo network follow these steps:

1. Download a [`zebra-crosslink` prebuilt binary for Milestone 3](https://github.com/ShieldedLabs/zebra-crosslink/releases/tag/v2.5.0) for your architecture. They are named as `<arch>_zebrad` for a headless binary, or `<arch>_viz_zebrad` for a binary with the integrated visualizer gui. <!-- xxx not currently up to date -->
2. Generate a default configuration with: `zebrad generate > myconfig.toml`
3. Edit the config for the demo network to configure the node to use the Crosslink testnet, connection parameters, and your node identity see example below.
4. Join the [Zcash global discord](https://discord.gg/CWSCEWvq4C), and we'll be on the stage.
5. Join the network with `zebrad --config ./myconfig.toml`. If you are using the `viz` binary, the gui should open right away. Congrats, you are now running a hybrid PoW/BFT finalizer node! 🎉
6. Post your `crosslink.public_address` to the discord chat.

<!-- The above 2-6 steps should be automated so when you just run `cargo run` it does all of that. Except step 4. -->

We can manually add your node to the roster. Before that your node should still be able to sync and observe the dual-chain progress.

## Configuration

The default configuration that `zebrad generate` produces is not yet tailored this workshop demo. You need to make these modifications:

### Basic Zebra Configuration

First, we change the network settings:

```toml
[network]
network = "Testnet"
initial_testnet_peers = [
    "80.78.31.51:8233",
    "80.78.31.32:8233",
]
```

Any other settings in `[network]` can remain unchanged.

Next we instruct zebra to only connect to Crosslink networks and not other Zcash testnets by adding this section:

```
[network.testnet_parameters]
network_magic = [67, 108, 84, 110]
slow_start_interval = 0
```

### Crosslink-specific settings

Now for the Crosslink goodies!

For this demo we have overloaded the meaning of `public_address` which serves two different purposes: one is the node's (pretend) cryptographic identity, and one is only relevant if you want to accept incoming TCP connections to your publicly routable IP address.

If you DO want to accept incoming TCP connections, make sure your IP address is publicly routable then use that address and port. You must also set the `listen_address` as in the example below (with the same port).

If you DO NOT want/need/care about accepting incoming TCP connections, use the IP address 7.7.7.7 and pick a random port between 10,000 and 20,000.

```
[crosslink]
# An example for a participant without a public IP address:
public_address = "/ip4/7.7.7.7/tcp/<YOUR PORT>"

# An example for a participant who wants incoming TCP connections:
#public_address = "/ip4/<YOUR PUBLIC IP>/tcp/<YOUR PORT>"
#listen_address = "/ip4/0.0.0.0/tcp/<YOUR PORT>"

# These peers are necessary to bootstrap:
malachite_peers = [
    "/ip4/80.78.31.51/tcp/8234",
    "/ip4/80.78.31.32/tcp/8234",
    "/ip4/5.5.5.5/tcp/5555",
]
```

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

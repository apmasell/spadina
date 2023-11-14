# Background for Developers
This provides some background for anyone wanting to modify the Spadina
software. Extending the server will likely require setting up a server, so
reading [Background for Server Administrators](background-admin.md) is
recommended.

## Developer Setup
The game is build in Rust. Install the Rust toolchain using
[rustup](https://rustup.rs/).

You may want to install the Diesel database interface using:

    cargo install diesel_cli

You will need the WASM toolchain:

    rustup target add wasm32-unknown-unknown
    cargo install wasm-bindgen-cli

On Debian/Ubuntu Linux, install:

    sudo apt-get install gcc pkg-config openssl libasound2-dev cmake build-essential python3 libfreetype6-dev libexpat1-dev libxcb-composite0-dev libssl-dev libx11-dev libpq-dev libudev-dev libmysqlclient-dev

On Windows, install the vcpkg program and install:

    vcpkg.exe install openssl:x64-windows-static-md

The build using:

    cargo build --target wasm32-unknown-unknown -p spadina-client
    wasm-bindgen --target web target/wasm32-unknown-unknown/debug/spadina-client.wasm  --out-dir server/ --no-typescript
    cargo build

## The Server/Client Split
Spadina was designed to meet a few goals:

- security: malicious clients, malicious servers, and malicious realms should not disrupt functioning of the system
- consistency: players in the same realm should have a consistent experience; this means minimal glitching/teleporting. This necessarily requires keeping bandwidth demands low.
- resilience:  like any distributed system, parts of the system will be in a failed state and the surrounding infrastructure needs to continue operating
- limit cheating: make sure players with technical skills can't introspect the puzzle state

To this end, the sever/client split is somewhat different than most MMOs. The
authoritatively server synchronises all behaviour in the game (rather than
allowing clients to predict what will happen and try to reconcile on the
server). To make the latency acceptable, the game dispenses with real-time
navigation and physics.

## Content Addressable Memory
Each asset required is stored in Message Pack format and is identified by a
hash of its contents. This means that every asset has a true name and can query
its peers for the asset and validate the asset returned.

This also allows the federated network to function a bit like a BitTorrent
swarm: servers can swap assets and discover other peers to spread the load.

## Capabilities
A federated network will be impossible to upgrade in a coordinated way. In the
spirit of openness, people should be able to create new experimental features
and test them in the wild without harming the network.

To accomplish this, features are associated with a _capability_. Each
capability is just a string with the name of a feature. When a realm is
created, all the capabilities it uses are written into the realm's description.
When a server gets a realm description, it can check if it has the capability
set needed to load this realm. The client can do the same. This allows a new
features to be added to an experimental server and client, be used and have
those assets distributed to incompatible servers without causing a problem.

Servers still need to process all assets in a realm, so a server must support
all the capabilities required by a realm even if the effects are mostly client
side. A client accessing a realm on a remote server should be able to use
capabilities its local server doesn't support since the local server doesn't
need to read the assets directly.

## Applets
In a strange way, each realm is a small program running on the server. The
realm's asset functions as the code for the realm and the state information is
the program's memory. This idea became extended out to allow non-realm things
to run on the server as long as they can behave in a similar enough way. In
particular, the game editor can become an in-network object.

Each applet has a front-end and back-end. The front-end runs on the client and
the back-end runs on the server. They can communicate to each other in their
own specific network protocol that's transported over the client-server
connection. This means that some functions, such as database interaction,
access control, and federation are abstracted out of the applets.

The front-end has an API with the following constraints:

- it may receive an update from the back-end at any time
- it may recieve input from the user at any time
- it can generate and manage a user interface and callbacks
- it can have internal state, but this state is not persisted

The back-end has an API with the following constraints:

- it may recieve an update from any front-end (with an associated player identifier) at any time
- it can set a watch dog timer and be notified if it elapses
- it can have internal state and this state will be persisted to the server's database
- it must be able to reload itself from the persisted state (and a realm asset)
- the persisted state must convertible to JSON, JSONB, or Message Pack

### Weird Philosophy Tangent
In a strange way, this game has become a reimaging of modern web architecture.
Ooops. The client is in some way, a browser, providing a set of UI primitives.
As a browser provides UI primitive based on the DOM, Spadina is attempting to
provide UI primitives for 3D graphics and UI widgets. Unlike a normal web
client/server application, the communication is an asynchronous stream of
messages, rather than RPC-like behaviour. The implementations of the front and
back-ends are intended to be built into the client and server directly, but
that's not a hard requirement and it would be completely sensible to allow
implementing them in WASM and allow dynamic loading.

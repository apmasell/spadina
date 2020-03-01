# Puzzleverse

Puzzleverse is a massively multiplayer online puzzle solving game where players
can create their own puzzle-driven stories.

https://runyourown.social/

It is still in early development and not ready for users.

## Client Setup on Windows
## Client Setup on MacOS
## Client Setup on Ubuntu
TODO
## Client Setup on Linux
You will need Vulkan drivers installed. On Ubunut/Debian, do:

    sudo apt-get install mesa-vulkan-drivers

## Server Setup
The recommended server is Ubuntu since nearly all the steps can be managed by the package manager. To start, create a server (either bare metal or a virtual machine) and install the operating system. You will need to have a domain name associated with this server. For these instructions, it is assumed to be _example.com_.

### Certificates on Ubuntu
You will need to set up an SSL certificate to allow your server to be public. You can generate a certificate using [Let's Encrypt](https://letsencrypt.org/) as follows. To do so, run:

    sudo apt-get install certbot
    sudo certbot certonly --standalone

You will need to create a PKCS12 file from the certificate:

    openssl pkcs12 -export -out /etc/puzzleverse/cert.pfx -inkey /etc/letsencrypt/live/${SERVER_NAME}/privkey.pem -in /etc/letsencrypt/live/${SERVER_NAME}/cert.pem -certfile /etc/letsencrypt/live/${SERVER_NAME}/fullchain.pem

This must be done when `certbot` updates the signature, so it maybe best to put this in a systemd cron job:

    sudo cp puzzleverse-pkcs12.* /lib/systemd/system
    sudo systemctl daemon-reload

This will work on all systemd-based Linux distributions, including Debian and Ubuntu.

### Certificates on Other Operating Systems
For other operating systems, follow the [Certbot instructions](https://certbot.eff.org/instructions), selecting _None of the above_ for the software and then using the _Yes, my web server is not currently running on this machine_ path in the instructions.


### Server Installation on Ubuntu
These instructions are meant for a Debian or Ubuntu server. It is possible to install on other Linux distributions or operating systems.

First, create a directory for the server configuration:

    sudo mkdir /etc/puzzleverse

You will need the [PostgrSQL](https://www.postgresql.org/) database. On Debian/Ubuntu, invoke:

    sudo apt-get install postgesql

Generate a random database password:

    DB_PASS=$(openssl rand -base64 32)
    echo $DB_PASS

Then, create a new database for Puzzleverse. On Debian/Ubuntu, invoke:

    sudo -u postgres psql -c "CREATE ROLE puzzleverse PASSWORD '"${DB_PASS}"' LOGIN; CREATE DATABASE puzzleverse OWNER puzzleverse;"

For other operating systems, connect to the database as the administrator and then, substituting `DB_PASS` with the real password, issue:

    CREATE ROLE puzzleverse PASSWORD 'DB_PASS' LOGIN;
    CREATE DATABASE puzzleverse OWNER puzzleverse;


TODO authconfig

Now, install the server itself. If you have downloaded a binary:

    sudo cp puzzleverse-server /usr/bin
    sudo cp puzzleverse.service /lib/systemd/system

On Ubuntu, you can install from a PPA:

    sudo apt-add-repository -y ppa:puzzleverse/ppa
    sudo apt-get update
    sudo apt-get install puzzleverse-server

Skip this step iIf using a systemd-based Linux distribution, including Debian, copy the default configuration as follows:

    sudo cp defaults /etc/default/puzzleverse
    ./generate-config | sudo tee /etc/default/puzzleverse
    sudo systemctl daemon-reload
    sudo systemctl enable puzzleverse
    sudo systemctl start puzzleverse

And that should be all!
    puzzleverse-server -a method:/etc/puzzleverse/auth.cfg -d postgres://puzzleverse:test@localhost/puzzleverse -j ${JWT_SECRET} --ssl /etc/puzzleverse/cert.pfx



TODO


## Developer Setup
The game is build in Rust. Install the Rust toolchain using
[rustup](https://rustup.rs/). Then install the dependencies for Godot:

    apt install llvm-dev libclang-dev clang

You may want to install the Diesel database interface using:

    cargo install diesel_cli

You will need the WASM toolchain:

    rustup target add wasm32-unknown-unknown
    cargo install wasm-bindgen-cli

The build using:

    cargo build --target wasm32-unknown-unknown -p puzzleverse-client
    wasm-bindgen --target web target/wasm32-unknown-unknown/debug/puzzleverse-client.wasm  --out-dir server/ --no-typescript
    cargo build

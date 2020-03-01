# Background for Server Administrators
Running a Spadina server allows you to create a control a community.
It is part technical and part social. You will be responsible for how players
on your server conduct themselves when visiting other servers and other
administrators can choose to cut _all_ your players off from the network.

This guide will cover mostly the technical aspects and the tools available to
manage your community. For information on running a community, the [Run Your
Own Social](https://runyourown.social/) guide is recommended reading.

## Getting a Server
You will need:

- a Linux or Windows server that can run a PostgreSQL database.
- a domain name
- an SSL certificate

The server RAM and CPU requirements are quite modest. The server will need to
cache in-game assets, so a large amount of disk is desirable.

Using a cloud hosting provider such as AWS or Azure should be quite sensible.
Since the setup procedures for these services are somewhat different, this
guide does not cover provisioning the server. This assumes you already have a
login to a Linux terminal.

## Certificates on Ubuntu
You will need to set up an SSL certificate to allow your server to be public.
You can generate a certificate using [Let's Encrypt](https://letsencrypt.org/)
as follows. To do so, run:

    sudo apt-get install certbot
    sudo certbot certonly --standalone

You will need to create a PKCS12 file from the certificate:

    openssl pkcs12 -export -out /etc/spadina/cert.pfx -inkey /etc/letsencrypt/live/${SERVER_NAME}/privkey.pem -in /etc/letsencrypt/live/${SERVER_NAME}/cert.pem -certfile /etc/letsencrypt/live/${SERVER_NAME}/fullchain.pem

This must be done when `certbot` updates the signature, so it maybe best to put
this in a systemd cron job:

    sudo cp spadina-pkcs12.* /lib/systemd/system
    sudo systemctl daemon-reload

This will work on all systemd-based Linux distributions, including Debian and Ubuntu.

## Certificates on Other Operating Systems
For other operating systems, follow the [Certbot
instructions](https://certbot.eff.org/instructions), selecting _None of the
above_ for the software and then using the _Yes, my web server is not currently
running on this machine_ path in the instructions.


## Server Installation on Debian or Ubuntu
These instructions are meant for a Debian or Ubuntu server. It is possible to
install on other Linux distributions or operating systems.

First, create a directory for the server configuration:

    sudo mkdir /etc/spadina

You will need the [PostgrSQL](https://www.postgresql.org/) database. On Debian/Ubuntu, invoke:

    sudo apt-get install postgesql

Generate a random database password:

    DB_PASS=$(openssl rand -base64 32)
    echo $DB_PASS

Then, create a new database for Spadina. On Debian/Ubuntu, invoke:

    sudo -u postgres psql -c "CREATE ROLE spadina PASSWORD '"${DB_PASS}"' LOGIN; CREATE DATABASE spadina OWNER spadina;"

For other operating systems, connect to the database as the administrator and then, substituting `DB_PASS` with the real password, issue:

    CREATE ROLE spadina PASSWORD 'DB_PASS' LOGIN;
    CREATE DATABASE spadina OWNER spadina;

Now, install the server itself. If you have downloaded a binary:

    sudo cp spadina-server /usr/bin
    sudo cp spadina.service /lib/systemd/system

On Ubuntu, you can install from a PPA:

    sudo apt-add-repository -y ppa:spadina/ppa
    sudo apt-get update
    sudo apt-get install spadina-server

If using a systemd-based Linux distribution, including Debian, copy the default configuration as follows:

    sudo systemctl daemon-reload
    sudo systemctl enable spadina
    sudo systemctl start spadina

Create `/etc/spadina.config` and we will modify this file to match your
needs:

```
{
  "asset_store": ...,
  "authentication": ...,
  "bind_address": null,
  "certificate": null,
  "database_url": "postgres://spadina:DB_PASS@localhost/spadina",
  "default_realm": ...,
  "name": ...,
  "unix_socket": "/var/run/spadina.socket"
}
```

First, `"name"` should be set to the full name for your server (_e.g._,
`spadina.example.com`). If this name is incorrect, federation will break.

Make sure to replace `DB_PASS` in `"database_url"` with the real database
password.

An SSL certificate is required to make Spadina work. There are two ways to
make this happen:

- create a certificate in PCKS12 format and set `"certificate"` to be the path
  to this file
- set up a reverse proxy

When using the certificate, the server will automatically bind to port 443 to
accept secure connections. When using the reverse proxy, you can change the
`"bind_address"` to be the port where the server should run (_e.g._,
`127.0.0.1:8080`). If you wish to use a reverse proxy, see [Using Nginx as a
Reverse Proxy](#nginx-reverse-proxy).

The server needs a place to store assets from other servers and `"asset_store"`
is where that configuration goes. Assets can be stored on local disk or in a
cloud storage system such as S3.

To store assets on local disk, use:

```
"asset_store": { "type": "FileSystem ", "directory": "/path/to/assets" },
```

and make sure that the directory can be written to by the user the server runs
as.

If running inside Google Cloud, the Google Cloud Storage can be used. Create a
bucket and then set:

```
"asset_store": { "type": "GoogleCloud", bucket: "your-bucket-name" },
```

If using S3 or an S3-compatible service (_e.g._ Minio), create a bucket and use:

```
"asset_store": {
  "type": "S3",
  "bucket": "your-bucket-name",
  "region": "us-east-1",
  "access_key": "ASDAFLSDALKG",
  "secret_key": "BGKLEGRLKW"
}
```

The server will need assets to initially start the game. You must pick an asset
pack and load it into your asset store. The asset pack will also include a
command to create a home realm.

TODO: Download asset pack from XXX. If using S3 or Google Cloud, upload all the asset files into the bucket. For local installation, run:

```
spadina-cli install-assets asset-pack.zip /path/to/assets
```

You will also need to install at least one realm asset to use as the home
realm. Set `"default_realm"` to the correct asset ID for you asset pack.

Your players will need to log in and the `"authentication"` setting controls
how that happens. There are two authentication mechanisms:

- password-based where the players send a password to the server and it checks
  against some kind of database
- OpenID Connect where the player is forwarded to an external service to do the
  authentication

Although OpenID Connect is more work to setup, it can be easier for a public
site since the database has no sensitive information in it and players can use
their existing Google, Facebook, or other always-logged-in account.

Some passwords stores can use one-time-passwords (OTPs). These are the
ever-changing 6-digit passwords used in some two-factor authentication systems.
Players will have to have an application to generate them (_e.g._, Google
Authenticator or a Yubikey).

| Method | Type | Configuration | Details |
|--------|------|---------------|---------|
| Manually-managed OTP database | OTP | `{ "type": "DatabaseOTPs " }` | This uses OTPs stored in a database. Multiple OTPs can be assigned to a user and the administrator must manage the OTPs manually. See [managing the OTP database](#otpdb) |
| LDAP Server | Password | `{ "type": "LDAP", "url": "ldaps://...", "bind_dn": "...", "bind_pw": "...", "search_base": "...", "account_attr": "..." }` | Uses an LDAP server (such as ActiveDirectory or OpenLDAP) as a password store. The LDAP administrator should create an account for Spadina to do searching as `"bind_dn"` and with the password in `"bind_pw"`. `"account_attr"` is the name of the attribute that will be the player's login (usually `"uid"` for OpenLDAP and `"sAMAccountName"` for ActiveDirectory. |
| Multiple OpenID Connect services | OpenId Connect | `{ "type": "MultipleOpenIdConnect", "providers": [ {"provider": {"type": "Google" }, "client_id": "...", "client_secret": ...}, ...] }` | Works much like Open ID Connect, but with a choice of providers. See the Open ID Connect information below. |
| Fixed OTPs | OTP | `{ "type": "OTPs", "users": { "andre": "aslkjasdklask", ...} }` | Uses a fixed list of OTPs for each user. Updating this list requires restarting the server, so this method is not recommended for production. |
| Single OpenID Connect service | Open ID Connect | `{ "type": "OpenIdConnect", "provider": {"type": "Google"}, "client_id": "...", "client_secret": "..." }` | Use a single OpenID Connect service for authentication. See details about OpenID Connect below. |
| Fixed Passwords | Password | `{ "type": "Passwords", "users": { "andre": "password", ...} }` | Uses exact passwords provided in the configuration file. *Do not use in production.* This is insecure and meant for debugging. |
| phpBB Forum | Password | `{ "type": "PhpBB", "connection": "...", "database": "..." }` | Uses an existing phpBB forum for passwords. This requires a database connection to the same one used by the forum. `"database"` is `"MySQL"` or `"PostgreSQL"` and `"connection"` is the URL to the database. Any account that is locked in phpBB will also be locked in Spadina. |
| Myst Online: Uru Live server | Password | `{ "type": "Uru", "connection": "..." }` | Uses a Myst Online server as a source for passwords. `"connection"` is the URL of the MOUL database. |

For the OpenID Connect services, the server must be registered with the
provider. When registering, a redirect/return/callback URL must be provided.
This will be `https://spadina.example.com/api/auth/oidc/auth` where
`spadina.example.com` is the name of your server. The registration process
will provide a client ID and client secret, which must be placed in the
configuration file.

| Service | Type | Registration Instructions |
|---------|------|---------------------------|
| Apple |`{"type": "Apple"}` | [Register Apps in the Apple Developer Portal](https://auth0.com/docs/connections/apple-siwa/set-up-apple) |
| Facebook |`{"type": "Facebook"}` | [Create an App](https://developers.facebook.com/docs/development/create-an-app) using a _Consumer_ app and *only step 1* of [Facebook Login for the Web](https://developers.facebook.com/docs/facebook-login/web) |
| Google | `{"type": "Google"}` | [Setting up OAuth 2.0](https://developers.google.com/identity/protocols/oauth2/openid-connect) |
| LinkedIn |`{"type": "LinkedIn"}` |
| Microsoft | `{ "type": "Microsoft", "tenant": null }` | [Register an application with the Microsoft identity platform](https://docs.microsoft.com/en-us/azure/active-directory/develop/quickstart-register-app) Microsoft supports individual organisation registration (tenants). If you have selected one, enter it in `"tenant"`. If `"tentant"` is null, Spadina will use `"common"`, which corresponds to _Accounts in any organizational directory and personal Microsoft accounts_. |
| Other | `{ "type": "Custom", "url": ... }` | For using a service not listed here. The service must supported OpenID Connect with Discovery. The URL provided will be probed for `/.well-known/openid-configuration` to discover its configuration. |


Start your server using:

```
systemctl start spadina
```

Check that the server is online by visiting the web site for it.

And that should be all!

<a name="nginx-reverse-proxy">
### Using Nginx as a Reverse Proxy
TODO

## Managing Server Access
Much like individual players can choose who access their realms and who can
send them direct messages, the server administrator can also set access rules.
They server administrators can set:

- which players from other servers can access this server
- which players from other servers can send direct messages to players on this server
- which players on this server can update these rules

The rules are combined with the player's personal rules. So, if a remote player
is blocked at a server level, there is no way for an individual player to still
allow that player access to their realms.

### Unix Socket Access
If you've managed to lock yourself out of the house, there's no need to be
embarrassed. Normally, clients connect over the web and go through an
authentication process. It is also possible to access Spadina _without_
authentication through a UNIX socket. In the Spadina configuration, set
`"unix_socket": "/var/run/spadina.socket"`.

This will create a file named `/var/run/spadina.socket`. You can now start a
client and use the path to that directory as the server name. The client will
connect to the server with no authentication. This is secure because the client
needs to gain access to the server. The client can choose, when connected via
the socket, whether the player can change any ACL (including the ACLs of other
player's realms).

If you cannot run the client on the server directly (_e.g._, its in a cloud
provider), you can forward the socket using SSH:

```
ssh -R /srv/spadina/.spadina.socket:/home/andre/.spadina.socket spadina@example.com
```

## Starting the Journey
When a new player joins your server, they will only be allowed to access their
home realm. Their home realm must have a trigger to _debut_ the player deciding
that they are ready to interact with the outside world. As the administrator,
you can list possible realms for your players. This is part of the
configuration of the server and the realm must be a train-car realm.

## Train-Car Realms
The server administrator can add realm descriptions to use as train cars. The
server will choose realms the player has not played as train cars. Not all
realms can be used as train cars (they must have a link to the next car). Once
added, train cars cannot be deleted. When adding a realm, the administrator can
choose if this realm is an appropriate first realm for a player. If multiple
realms are available as first realms, the system will choose one randomly. A
realm that is marked as appropriate for a first realm can also be used for
non-first train cars.

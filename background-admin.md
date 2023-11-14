# Background for Server Administrators
Running a Spadina server allows you to create and shepard a community.
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
name = spadina.example.com
unix_socket = /var/run/spadina.socket
```

First, `"name"` should be set to the full name for your server (_e.g._,
`spadina.example.com`). If this name is incorrect, federation will break.

An SSL certificate is required to make Spadina work. There are two ways to
make this happen:

- create a certificate in PCKS12 format and set `certificate = "/path/to/certificate.pcks12"`
- set up a reverse proxy

When using the certificate, the server will automatically bind to port 443 to
accept secure connections. When using the reverse proxy, you can change the
`bind_address = "127.0.0.1:8080"` to be the port where the server should run.
If you wish to use a reverse proxy, see [Using Nginx as a Reverse
Proxy](#nginx-reverse-proxy).

The server needs a place to store assets from other servers and `[asset_store.type]`
is where that configuration goes. Assets can be stored on local disk or in a
cloud storage system such as S3.

To store assets on local disk, use:

```
[asset_store.file_system]
directory = "/path/to/assets"
```

and make sure that the directory can be written to by the user the server runs
as.

If running inside Google Cloud, the Google Cloud Storage can be used. Create a
bucket and then set:

```
[asset_store.google_cloud]
bucket = "your-bucket-name"
```

If using S3 or an S3-compatible service (_e.g._ Minio), create a bucket and use:

```
[asset_store.s3]
bucket = "your-bucket-name",
region = "us-east-1",
access_key = "ASDAFLSDALKG",
secret_key = "BGKLEGRLKW"
```

The server will need assets to initially start the game. You must pick an asset
pack and load it into your asset store. The asset pack will also include a
command to create a home realm.

Your players will need to log in and the `[authentication]` setting controls
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

For authentication using a database, a database connection URL for `sqlite://`,
`postgresql://`, or `mysql://` may be used.

See [supported authentication mechanisms](#supported-auth) for the configuration.

Start your server using:

```
systemctl start spadina
```

Check that the server is online by visiting the web site for it.

And that should be all!

<a name="supported-auth">
### Suported Authentication

#### Manually-managed OTP database (OTP)
Uses one-time-passwords stored in a database. Multiple OTPs can be assigned to a user and the administrator must manage the OTPs manually.
```
[authentication]
database_otps = "sqlite://path/to/db.sqlite"
```

#### LDAP Server (Password)
Uses an LDAP server (such as ActiveDirectory or OpenLDAP) as a password store. The LDAP administrator should create an account for Spadina to do searching as `bind_dn` and with the password in `bind_pw`. `account_attr` is the name of the attribute that will be the player's login (usually `"uid"` for OpenLDAP and `"sAMAccountName"` for ActiveDirectory.)

The `admin_query` and `create_query` values are optional. If provided, they will determine if a players has administrative and upload rights, respectively. If absent, all users will have those privileges.

```
[authentication.ldap]
account_attr = "uid"
admin_query = "(&(objectClass=user)(memberof=CN=Administrators,OU=Users,DC=example,DC=com))"
bind_dn = "spadina"
bind_pw = "secret_password"
create_query = "(&(objectClass=user)(memberof=CN=Creatives,OU=Users,DC=example,DC=com))"
search_base = "ou=Users"
server_url = "ldaps://ldap.example.com"
user_query = "(objectClass=user)"
```

#### OpenID Connect (OpenID)
Allows connecting to one or more OpenID Connect-compatible services. 
For the OpenID Connect services, the server must be registered with the
provider. When registering, a redirect/return/callback URL must be provided.
This will be `https://spadina.example.com/api/auth/oidc/auth` where
`spadina.example.com` is the name of your server. The registration process
will provide a client ID and client secret, which must be placed in the
configuration file.

```
[authentication.open_id_connect]
connection = "sqlite://etc/spadina/open_id.db"
registration = "Invite"

[[authentication.open_id_connect.providers]]
client_id = "whatever_id"
client_secret = "whatever_secret"
provider = "Google"
```

For the OpenID Connect services, the server must be registered with the
provider. When registering, a redirect/return/callback URL must be provided.
This will be `https://spadina.example.com/api/auth/oidc/auth` where
`spadina.example.com` is the name of your server. The registration process
will provide a client ID and client secret, which must be placed in the
configuration file.

The `[[authentication.open_id_connect.providers]]` section can be repeated for
each of the services available.

| Service | Type | Registration Instructions |
|---------|------|---------------------------|
| Apple |`provider = "apple"` | [Register Apps in the Apple Developer Portal](https://auth0.com/docs/connections/apple-siwa/set-up-apple) |
| Facebook |`provider = "facebook"` | [Create an App](https://developers.facebook.com/docs/development/create-an-app) using a _Consumer_ app and *only step 1* of [Facebook Login for the Web](https://developers.facebook.com/docs/facebook-login/web) |
| Google | `provider = "google"` | [Setting up OAuth 2.0](https://developers.google.com/identity/protocols/oauth2/openid-connect) |
| LinkedIn |`provider = "LinkedIn"` |
| Microsoft | `provider = "microsoft"` or `provider = { microsoft_tenant = "00000000-0000-0000-0000-000000000000"` | [Register an application with the Microsoft identity platform](https://docs.microsoft.com/en-us/azure/active-directory/develop/quickstart-register-app) Microsoft supports individual organisation registration (tenants). If you have selected one, enter it using the second syntax. When the first syntax is used, Spadina will use `"common"`, which corresponds to _Accounts in any organizational directory and personal Microsoft accounts_. |
| Other | `provider = { url = "https://oidc.whatever.com", name = "Whatever" }` | For using a service not listed here. The service must supported OpenID Connect with Discovery. The URL provided will be probed for `/.well-known/openid-configuration` to discover its configuration. |

#### Fixed OTPs (OTP)
Uses a fixed list of OTPs for each user. Updating this list requires restarting the server, so this method is not recommended for production.

```
[authentication.otps]
andre = "JEQGI33OE52CA23ON53SA53IMF2CAZDBORQSAYLDOR2WC3DMPEQGY2LWMVZSA2DFOJSQ===="
claidi = "KNQW2ZJAMZXXEIDUNBUXGIDPNZSSA5DPN4======"
```
| Single OpenID Connect service | Open ID Connect | `{ "type": "OpenIdConnect", "provider": {"type": "Google"}, "client_id": "...", "client_secret": "..." }` | Use a single OpenID Connect service for authentication. See details about OpenID Connect below. |

#### Fixed Passwords (Password)
Uses exact passwords provided in the configuration file. *Do not use in production.* This is insecure and meant for debugging.

```
[authetnication.passwords]
andre = "notsecure"
```

#### phpBB Forum (Password)
Uses an existing phpBB forum for passwords. This requires a database connection to the same one used by the forum. Any account that is locked in phpBB will also be locked in Spadina.

```
[authetnication]
php_bb = "mysql://phpbb_user:secret@localhost/phpbb_db"
```

#### Myst Online: Uru Live server (Password)
Uses a Myst Online server as a source for passwords. Only PostgreSQL is supported.

```
[authentication]
uru = "postgresql://uru_user:secret@localhost/moul"
```

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

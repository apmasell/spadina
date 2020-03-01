CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE FUNCTION gen_calendar_id() RETURNS bytea 
LANGUAGE plpgsql
  AS
$$
DECLARE 
BEGIN
 RETURN gen_random_bytes(10);
END;
$$;

CREATE TABLE BannedPeers (
    ban jsonb UNIQUE NOT NULL,
    PRIMARY KEY (ban)
);

CREATE TABLE Announcement (
    id serial PRIMARY KEY NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    "when" jsonb NOT NULL,
    realm jsonb NOT NULL,
    "public" boolean NOT NULL
);

CREATE TABLE Player (
    id serial PRIMARY KEY NOT NULL,
    name text NOT NULL,
    debuted boolean NOT NULL,
    avatar jsonb NOT NULL,
    message_acl jsonb NOT NULL,
    online_acl jsonb NOT NULL,
    new_realm_access_acl jsonb NOT NULL,
    new_realm_admin_acl jsonb NOT NULL,
    reset boolean NOT NULL DEFAULT FALSE,
    calendar_id bytea NOT NULL DEFAULT gen_calendar_id(),
    last_login timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX player_name ON player(name);
CREATE INDEX player_calendar_id ON player(calendar_id);

CREATE TABLE LocalPlayerChat (
    sender int4 NOT NULL,
    recipient int4 NOT NULL,
    created timestamp WITH time zone NOT NULL,
    body jsonb NOT NULL,
    PRIMARY KEY (sender, recipient, created),
    CONSTRAINT local_playerchat_sender_player_id FOREIGN KEY (sender) REFERENCES Player (id),
    CONSTRAINT local_playerchat_recipient_player_id FOREIGN KEY (recipient) REFERENCES Player (id)
);

CREATE INDEX localplayerchat_recipient ON localplayerchat(recipient);
CREATE INDEX localplayerchat_by_timestamp ON localplayerchat (sender, recipient, created);

CREATE TABLE LocalPlayerLastRead (
    sender int4 NOT NULL,
    recipient int4 NOT NULL,
    "when" timestamp WITH time zone NOT NULL,
    PRIMARY KEY (sender, recipient),
    CONSTRAINT local_playerlastread_sender_player_id FOREIGN KEY (sender) REFERENCES Player (id),
    CONSTRAINT local_playerlastread_recipient_player_id FOREIGN KEY (recipient) REFERENCES Player (id)
);

CREATE TABLE RemotePlayerChat (
    player int4 NOT NULL,
    inbound boolean NOT NULL,
    remote_player text NOT NULL,
    remote_server text NOT NULL,
    created timestamp WITH time zone NOT NULL,
    body jsonb NOT NULL,
    PRIMARY KEY (remote_player, remote_server, created, player, inbound),
    CONSTRAINT remote_playerchat_player_id FOREIGN KEY (player) REFERENCES Player (id)
);

CREATE INDEX remoteplayerchat_by_timestamp ON remoteplayerchat (player, remote_server, remote_player, created);

CREATE TABLE RemotePlayerLastRead (
    player int4 NOT NULL,
    remote_player text NOT NULL,
    remote_server text NOT NULL,
    "when" timestamp WITH time zone NOT NULL,
    PRIMARY KEY (player, remote_player, remote_server),
    CONSTRAINT remote_playerlastread_player_id FOREIGN KEY (player) REFERENCES Player (id)
);

CREATE TABLE Realm (
    id serial PRIMARY KEY NOT NULL,
    train int4,
    name text NOT NULL,
    owner int4 NOT NULL,
    asset text NOT NULL,
    state jsonb,
    settings jsonb NOT NULL,
    access_acl jsonb NOT NULL,
    admin_acl jsonb NOT NULL,
    in_directory boolean NOT NULL,
    seed int4 NOT NULL,
    solved boolean NOT NULL DEFAULT false,
    created timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT realm_player_id FOREIGN KEY (owner) REFERENCES Player (id),
    CONSTRAINT realm_only_per_player UNIQUE (owner, asset)
);

CREATE INDEX realm_asset ON realm(asset);
CREATE INDEX realm_in_directory ON realm(in_directory);
CREATE INDEX realm_owner_train ON realm(owner, train);

SELECT diesel_manage_updated_at('Realm');

CREATE TABLE RealmChat (
    realm int4 NOT NULL,
    principal jsonb NOT NULL,
    created timestamp WITH time zone NOT NULL,
    body jsonb NOT NULL,
    PRIMARY KEY (principal, created, realm),
    CONSTRAINT realmchat_realm_id FOREIGN KEY (realm) REFERENCES Realm (id)
);

CREATE INDEX realmchat_by_timestamp ON realmchat (realm, created);

CREATE TABLE RealmAnnouncement (
    id serial PRIMARY KEY NOT NULL,
    realm int4 NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    "when" jsonb NOT NULL,
    "public" boolean NOT NULL,
    CONSTRAINT realmannouncement_realm_id FOREIGN KEY (realm) REFERENCES Realm (id)
);

CREATE TABLE RealmCalendarSubscription (
    realm int4 NOT NULL,
    player int4 NOT NULL,
    PRIMARY KEY (realm, player),
    CONSTRAINT realm_calendar_subscription_player_id FOREIGN KEY (player) REFERENCES Player (id),
    CONSTRAINT realm_calendar_subscription_realm_id FOREIGN KEY (realm) REFERENCES Realm (id)
);

CREATE TABLE ServerACL (
    category varchar(1) NOT NULL,
    acl jsonb NOT NULL,
    PRIMARY KEY (category)
);

CREATE TABLE AuthOIDC (
    name text NOT NULL,
    subject text NOT NULL,
    locked boolean NOT NULL DEFAULT false,
    issuer text,
    PRIMARY KEY (name)
);

CREATE TABLE AuthOTP (
    name text NOT NULL,
    code text NOT NULL,
    locked boolean NOT NULL DEFAULT false,
    PRIMARY KEY (name, code)
);

CREATE TABLE Bookmark (
    player int4 NOT NULL,
    value jsonb NOT NULL,
    PRIMARY KEY(player, value),
    CONSTRAINT bookmark_player_id FOREIGN KEY (player) REFERENCES Player (id)
);

CREATE TABLE PublicKey (
    player int4 NOT NULL,
    fingerprint text NOT NULL,
    public_key bytea NOT NULL,
    last_used timestamp WITH time zone,
    created timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (player, fingerprint),
    CONSTRAINT host_player_id FOREIGN KEY (player) REFERENCES Player (id)
);

CREATE TABLE RealmTrain (
   asset text NOT NULL,
   allowed_first boolean NOT NULL,
   PRIMARY KEY (asset)
);

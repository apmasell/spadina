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

CREATE TABLE Announcement (
    id serial PRIMARY KEY NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    "when" jsonb NOT NULL,
    location jsonb NOT NULL,
    "public" boolean NOT NULL
);

CREATE TABLE Player (
    id serial PRIMARY KEY NOT NULL,
    name text NOT NULL,
    avatar jsonb NOT NULL,
    message_acl jsonb NOT NULL,
    online_acl jsonb NOT NULL,
    default_location_acl jsonb NOT NULL,
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

CREATE TABLE Location (
    id serial PRIMARY KEY NOT NULL,
    name text NOT NULL,
    owner int4 NOT NULL,
    descriptor jsonb NOT NULL,
    state jsonb NOT NULL,
    acl jsonb NOT NULL,
    visibility smallint NOT NULL,
    visibility_changed timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp WITH time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT location_player_id FOREIGN KEY (owner) REFERENCES Player (id),
    CONSTRAINT location_only_per_player UNIQUE (owner, descriptor)
);

CREATE INDEX location_descriptor ON location(descriptor);
CREATE INDEX location_visibility ON location(visibility);

SELECT diesel_manage_updated_at('Location');

CREATE TABLE LocationChat (
    location int4 NOT NULL,
    principal jsonb NOT NULL,
    created timestamp WITH time zone NOT NULL,
    body jsonb NOT NULL,
    PRIMARY KEY (principal, created, location),
    CONSTRAINT locationchat_location_id FOREIGN KEY (location) REFERENCES Location (id)
);

CREATE INDEX locationchat_by_timestamp ON locationchat (location, created);

CREATE TABLE LocationAnnouncement (
    id serial PRIMARY KEY NOT NULL,
    location int4 NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    "when" jsonb NOT NULL,
    "public" boolean NOT NULL,
    CONSTRAINT locationannouncement_location_id FOREIGN KEY (location) REFERENCES Location (id)
);

CREATE TABLE LocationCalendarSubscription (
    location int4 NOT NULL,
    player int4 NOT NULL,
    PRIMARY KEY (location, player),
    CONSTRAINT location_calendar_subscription_player_id FOREIGN KEY (player) REFERENCES Player (id),
    CONSTRAINT location_calendar_subscription_location_id FOREIGN KEY (location) REFERENCES Location (id)
);

CREATE TABLE ServerSetting (
    category varchar(1) NOT NULL,
    data jsonb NOT NULL,
    PRIMARY KEY (category)
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

CREATE VIEW PlayerSize AS
  SELECT SUM(pg_column_size(Location.*)) AS total, Player.name AS player
  FROM Player JOIN Location ON Player.id = Location.owner
  GROUP BY Player.name;

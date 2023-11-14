CREATE TABLE announcement (
    id int PRIMARY KEY NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    "when" blob NOT NULL,
    location blob NOT NULL,
    "public" boolean NOT NULL
);

CREATE TABLE player (
    id int PRIMARY KEY NOT NULL,
    name text NOT NULL,
    avatar blob NOT NULL,
    message_acl blob NOT NULL,
    online_acl blob NOT NULL,
    default_location_acl blob NOT NULL,
    reset boolean NOT NULL DEFAULT FALSE,
    calendar_id blob NOT NULL,
    last_login timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX player_name ON player(name);
CREATE INDEX player_calendar_id ON player(calendar_id);

CREATE TABLE local_player_chat (
    sender int NOT NULL,
    recipient int NOT NULL,
    created timestamp NOT NULL,
    body blob NOT NULL,
    PRIMARY KEY (sender, recipient, created),
    CONSTRAINT local_playerchat_sender_player_id FOREIGN KEY (sender) REFERENCES player (id),
    CONSTRAINT local_playerchat_recipient_player_id FOREIGN KEY (recipient) REFERENCES player (id)
);

CREATE INDEX localplayerchat_recipient ON local_player_chat(recipient);
CREATE INDEX localplayerchat_by_timestamp ON local_player_chat (sender, recipient, created);

CREATE TABLE local_player_last_read (
    sender int NOT NULL,
    recipient int NOT NULL,
    "when" timestamp NOT NULL,
    PRIMARY KEY (sender, recipient),
    CONSTRAINT local_playerlastread_sender_player_id FOREIGN KEY (sender) REFERENCES player (id),
    CONSTRAINT local_playerlastread_recipient_player_id FOREIGN KEY (recipient) REFERENCES player (id)
);

CREATE TABLE remote_player_chat (
    player int NOT NULL,
    inbound boolean NOT NULL,
    remote_player text NOT NULL,
    remote_server text NOT NULL,
    created timestamp NOT NULL,
    body blob NOT NULL,
    PRIMARY KEY (remote_player, remote_server, created, player, inbound),
    CONSTRAINT remote_playerchat_player_id FOREIGN KEY (player) REFERENCES player (id)
);

CREATE INDEX remoteplayerchat_by_timestamp ON remote_player_chat (player, remote_server, remote_player, created);

CREATE TABLE remote_player_last_read (
    player int NOT NULL,
    remote_player text NOT NULL,
    remote_server text NOT NULL,
    "when" timestamp NOT NULL,
    PRIMARY KEY (player, remote_player, remote_server),
    CONSTRAINT remote_playerlastread_player_id FOREIGN KEY (player) REFERENCES player (id)
);

CREATE TABLE location (
    id int PRIMARY KEY NOT NULL,
    name text NOT NULL,
    owner int NOT NULL,
    descriptor blob NOT NULL,
    state blob NOT NULL,
    acl blob NOT NULL,
    visibility smallint NOT NULL,
    visibility_changed timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT location_player_id FOREIGN KEY (owner) REFERENCES player (id),
    CONSTRAINT location_only_per_player UNIQUE (owner, descriptor)
);

CREATE INDEX location_descriptor ON location(descriptor);
CREATE INDEX location_visibility ON location(visibility);

CREATE TABLE location_chat (
    location int NOT NULL,
    principal blob NOT NULL,
    created timestamp NOT NULL,
    body blob NOT NULL,
    PRIMARY KEY (principal, created, location),
    CONSTRAINT locationchat_location_id FOREIGN KEY (location) REFERENCES location (id)
);

CREATE INDEX locationchat_by_timestamp ON location_chat (location, created);

CREATE TABLE location_announcement (
    id int PRIMARY KEY NOT NULL,
    location int NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    "when" blob NOT NULL,
    "public" boolean NOT NULL,
    expires timestamp NOT NULL,
    CONSTRAINT locationannouncement_location_id FOREIGN KEY (location) REFERENCES location (id)
);

CREATE TABLE location_calendar_subscription (
    location int NOT NULL,
    player int NOT NULL,
    PRIMARY KEY (location, player),
    CONSTRAINT location_calendar_subscription_player_id FOREIGN KEY (player) REFERENCES player (id),
    CONSTRAINT location_calendar_subscription_location_id FOREIGN KEY (location) REFERENCES location (id)
);

CREATE TABLE remote_calendar_subscription (
    owner text NOT NULL,
    server text NOT NULL,
    descriptor blob NOT NULL,
    player int NOT NULL,
    PRIMARY KEY (server, owner, descriptor, player),
    CONSTRAINT remote_calendar_subscription_player_id FOREIGN KEY (player) REFERENCES player (id)
);

CREATE TABLE server_setting (
    category varchar(1) NOT NULL,
    data blob NOT NULL,
    PRIMARY KEY (category)
);

CREATE TABLE bookmark (
    player int NOT NULL,
    value blob NOT NULL,
    PRIMARY KEY(player, value),
    CONSTRAINT bookmark_player_id FOREIGN KEY (player) REFERENCES player (id)
);

CREATE TABLE public_key (
    player int NOT NULL,
    fingerprint text NOT NULL,
    key blob NOT NULL,
    last_used timestamp,
    created timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (player, fingerprint),
    CONSTRAINT public_key_player_id FOREIGN KEY (player) REFERENCES player (id)
);

CREATE TABLE calendar_cache (
    player int NOT NULL,
    server text NOT NULL,
    calendar_entries blob NOT NULL,
    last_used timestamp,
    last_requested timestamp,
    last_updated timestamp,
    created timestamp NOT NULL,
    PRIMARY KEY (player, server),
    CONSTRAINT calendar_cache_player_id FOREIGN KEY (player) REFERENCES player (id)
);

CREATE VIEW player_size AS
  SELECT SUM(length(location.state) + length(location.acl)) AS total, player.name AS player
  FROM player JOIN location ON Player.id = location.owner
  GROUP BY player.name;

// @generated automatically by Diesel CLI.

diesel::table! {
    announcement (id) {
        id -> Int4,
        title -> Text,
        body -> Text,
        when -> Jsonb,
        location -> Jsonb,
        public -> Bool,
    }
}

diesel::table! {
    bookmark (player, value) {
        player -> Int4,
        value -> Jsonb,
    }
}

diesel::table! {
    localplayerchat (sender, recipient, created) {
        sender -> Int4,
        recipient -> Int4,
        created -> Timestamptz,
        body -> Jsonb,
    }
}

diesel::table! {
    localplayerlastread (sender, recipient) {
        sender -> Int4,
        recipient -> Int4,
        when -> Timestamptz,
    }
}

diesel::table! {
    location (id) {
        id -> Int4,
        name -> Text,
        owner -> Int4,
        descriptor -> Jsonb,
        state -> Jsonb,
        acl -> Jsonb,
        visibility -> Int2,
        visibility_changed -> Timestamptz,
        created -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    locationannouncement (id) {
        id -> Int4,
        location -> Int4,
        title -> Text,
        body -> Text,
        when -> Jsonb,
        public -> Bool,
    }
}

diesel::table! {
    locationcalendarsubscription (location, player) {
        location -> Int4,
        player -> Int4,
    }
}

diesel::table! {
    locationchat (principal, created, location) {
        location -> Int4,
        principal -> Jsonb,
        created -> Timestamptz,
        body -> Jsonb,
    }
}

diesel::table! {
    player (id) {
        id -> Int4,
        name -> Text,
        avatar -> Jsonb,
        message_acl -> Jsonb,
        online_acl -> Jsonb,
        default_location_acl -> Jsonb,
        reset -> Bool,
        calendar_id -> Bytea,
        last_login -> Timestamptz,
        created -> Timestamptz,
    }
}

diesel::table! {
    publickey (player, fingerprint) {
        player -> Int4,
        fingerprint -> Text,
        public_key -> Bytea,
        last_used -> Nullable<Timestamptz>,
        created -> Timestamptz,
    }
}

diesel::table! {
    remoteplayerchat (remote_player, remote_server, created, player, inbound) {
        player -> Int4,
        inbound -> Bool,
        remote_player -> Text,
        remote_server -> Text,
        created -> Timestamptz,
        body -> Jsonb,
    }
}

diesel::table! {
    remoteplayerlastread (player, remote_player, remote_server) {
        player -> Int4,
        remote_player -> Text,
        remote_server -> Text,
        when -> Timestamptz,
    }
}

diesel::table! {
    serversetting (category) {
        category -> Varchar,
        data -> Jsonb,
    }
}

diesel::joinable!(bookmark -> player (player));
diesel::joinable!(location -> player (owner));
diesel::joinable!(locationannouncement -> location (location));
diesel::joinable!(locationcalendarsubscription -> location (location));
diesel::joinable!(locationcalendarsubscription -> player (player));
diesel::joinable!(locationchat -> location (location));
diesel::joinable!(publickey -> player (player));
diesel::joinable!(remoteplayerchat -> player (player));
diesel::joinable!(remoteplayerlastread -> player (player));

diesel::allow_tables_to_appear_in_same_query!(
  announcement,
  bookmark,
  localplayerchat,
  localplayerlastread,
  location,
  locationannouncement,
  locationcalendarsubscription,
  locationchat,
  player,
  publickey,
  remoteplayerchat,
  remoteplayerlastread,
  serversetting,
);

// @generated automatically by Diesel CLI.

diesel::table! {
    announcement (id) {
        id -> Integer,
        title -> Text,
        body -> Text,
        when -> Binary,
        location -> Binary,
        public -> Bool,
    }
}

diesel::table! {
    bookmark (player, value) {
        player -> Integer,
        value -> Binary,
    }
}

diesel::table! {
    calendar_cache (player, server) {
        player -> Integer,
        server -> Text,
        calendar_entries -> Binary,
        last_used -> Nullable<Timestamp>,
        last_requested -> Nullable<Timestamp>,
        last_updated -> Nullable<Timestamp>,
        created -> Timestamp,
    }
}

diesel::table! {
    local_player_chat (sender, recipient, created) {
        sender -> Integer,
        recipient -> Integer,
        created -> Timestamp,
        body -> Binary,
    }
}

diesel::table! {
    local_player_last_read (sender, recipient) {
        sender -> Integer,
        recipient -> Integer,
        when -> Timestamp,
    }
}

diesel::table! {
    location (id) {
        id -> Integer,
        name -> Text,
        owner -> Integer,
        descriptor -> Binary,
        state -> Binary,
        acl -> Binary,
        visibility -> SmallInt,
        visibility_changed -> Timestamp,
        created -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    location_announcement (id) {
        id -> Integer,
        location -> Integer,
        title -> Text,
        body -> Text,
        when -> Binary,
        public -> Bool,
        expires -> Timestamp,
    }
}

diesel::table! {
    location_calendar_subscription (location, player) {
        location -> Integer,
        player -> Integer,
    }
}

diesel::table! {
    location_chat (location, principal, created) {
        location -> Integer,
        principal -> Binary,
        created -> Timestamp,
        body -> Binary,
    }
}

diesel::table! {
    player (id) {
        id -> Integer,
        name -> Text,
        avatar -> Binary,
        message_acl -> Binary,
        online_acl -> Binary,
        default_location_acl -> Binary,
        reset -> Bool,
        calendar_id -> Binary,
        last_login -> Timestamp,
        created -> Timestamp,
    }
}

diesel::table! {
    public_key (player, fingerprint) {
        player -> Integer,
        fingerprint -> Text,
        key -> Binary,
        last_used -> Nullable<Timestamp>,
        created -> Timestamp,
    }
}

diesel::table! {
    remote_calendar_subscription (owner, server, descriptor, player) {
        owner -> Text,
        server -> Text,
        descriptor -> Binary,
        player -> Integer,
    }
}

diesel::table! {
    remote_player_chat (player, inbound, remote_player, remote_server, created) {
        player -> Integer,
        inbound -> Bool,
        remote_player -> Text,
        remote_server -> Text,
        created -> Timestamp,
        body -> Binary,
    }
}

diesel::table! {
    remote_player_last_read (player, remote_player, remote_server) {
        player -> Integer,
        remote_player -> Text,
        remote_server -> Text,
        when -> Timestamp,
    }
}

diesel::table! {
    server_setting (category) {
        category -> Text,
        data -> Binary,
    }
}

diesel::joinable!(bookmark -> player (player));
diesel::joinable!(calendar_cache -> player (player));
diesel::joinable!(location -> player (owner));
diesel::joinable!(location_announcement -> location (location));
diesel::joinable!(location_calendar_subscription -> location (location));
diesel::joinable!(location_calendar_subscription -> player (player));
diesel::joinable!(location_chat -> location (location));
diesel::joinable!(public_key -> player (player));
diesel::joinable!(remote_calendar_subscription -> player (player));
diesel::joinable!(remote_player_chat -> player (player));
diesel::joinable!(remote_player_last_read -> player (player));

diesel::allow_tables_to_appear_in_same_query!(
  announcement,
  bookmark,
  calendar_cache,
  local_player_chat,
  local_player_last_read,
  location,
  location_announcement,
  location_calendar_subscription,
  location_chat,
  player,
  public_key,
  remote_calendar_subscription,
  remote_player_chat,
  remote_player_last_read,
  server_setting,
);

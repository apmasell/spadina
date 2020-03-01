// @generated automatically by Diesel CLI.

diesel::table! {
    announcement (id) {
        id -> Int4,
        contents -> Text,
        expires -> Timestamptz,
        event -> Bytea,
        realm -> Bytea,
    }
}

diesel::table! {
    authoidc (name) {
        name -> Text,
        subject -> Text,
        locked -> Bool,
        issuer -> Nullable<Text>,
    }
}

diesel::table! {
    authotp (name, code) {
        name -> Text,
        code -> Text,
        locked -> Bool,
    }
}

diesel::table! {
    bannedpeers (server) {
        server -> Text,
    }
}

diesel::table! {
    bookmark (player, asset) {
        player -> Int4,
        asset -> Text,
        kind -> Varchar,
    }
}

diesel::table! {
    localplayerchat (sender, recipient, created) {
        sender -> Int4,
        recipient -> Int4,
        created -> Timestamptz,
        body -> Text,
    }
}

diesel::table! {
    player (id) {
        id -> Int4,
        name -> Text,
        debuted -> Bool,
        waiting_for_train -> Bool,
        avatar -> Bytea,
        message_acl -> Bytea,
        online_acl -> Bytea,
        location_acl -> Bytea,
        new_realm_access_acl -> Bytea,
        new_realm_admin_acl -> Bytea,
        last_login -> Timestamptz,
        created -> Timestamptz,
    }
}

diesel::table! {
    publickey (player, name) {
        player -> Int4,
        name -> Text,
        public_key -> Bytea,
    }
}

diesel::table! {
    realm (id) {
        id -> Int4,
        principal -> Text,
        train -> Nullable<Int4>,
        name -> Text,
        owner -> Int4,
        asset -> Text,
        state -> Bytea,
        settings -> Bytea,
        access_acl -> Bytea,
        admin_acl -> Bytea,
        in_directory -> Bool,
        initialised -> Bool,
        seed -> Int4,
        created -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    realmchat (principal, created, realm) {
        realm -> Int4,
        principal -> Text,
        created -> Timestamptz,
        body -> Text,
    }
}

diesel::table! {
    realmtrain (asset) {
        asset -> Text,
        allowed_first -> Bool,
    }
}

diesel::table! {
    remoteplayerchat (remote_player, remote_server, created, player, state) {
        player -> Int4,
        state -> Varchar,
        remote_player -> Text,
        remote_server -> Text,
        created -> Timestamptz,
        body -> Text,
    }
}

diesel::table! {
    serveracl (category) {
        category -> Varchar,
        acl -> Bytea,
    }
}

diesel::joinable!(bookmark -> player (player));
diesel::joinable!(publickey -> player (player));
diesel::joinable!(realm -> player (owner));
diesel::joinable!(realmchat -> realm (realm));
diesel::joinable!(remoteplayerchat -> player (player));

diesel::allow_tables_to_appear_in_same_query!(
    announcement,
    authoidc,
    authotp,
    bannedpeers,
    bookmark,
    localplayerchat,
    player,
    publickey,
    realm,
    realmchat,
    realmtrain,
    remoteplayerchat,
    serveracl,
);

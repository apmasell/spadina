// @generated automatically by Diesel CLI.

diesel::table! {
    announcement (id) {
        id -> Int4,
        title -> Text,
        body -> Text,
        when -> Jsonb,
        realm -> Jsonb,
        public -> Bool,
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
    bannedpeers (ban) {
        ban -> Jsonb,
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
    player (id) {
        id -> Int4,
        name -> Text,
        debuted -> Bool,
        avatar -> Jsonb,
        message_acl -> Jsonb,
        online_acl -> Jsonb,
        new_realm_access_acl -> Jsonb,
        new_realm_admin_acl -> Jsonb,
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
    realm (id) {
        id -> Int4,
        train -> Nullable<Int4>,
        name -> Text,
        owner -> Int4,
        asset -> Text,
        state -> Nullable<Jsonb>,
        settings -> Jsonb,
        access_acl -> Jsonb,
        admin_acl -> Jsonb,
        in_directory -> Bool,
        seed -> Int4,
        solved -> Bool,
        created -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    realmannouncement (id) {
        id -> Int4,
        realm -> Int4,
        title -> Text,
        body -> Text,
        when -> Jsonb,
        public -> Bool,
    }
}

diesel::table! {
    realmcalendarsubscription (realm, player) {
        realm -> Int4,
        player -> Int4,
    }
}

diesel::table! {
    realmchat (principal, created, realm) {
        realm -> Int4,
        principal -> Jsonb,
        created -> Timestamptz,
        body -> Jsonb,
    }
}

diesel::table! {
    realmtrain (asset) {
        asset -> Text,
        allowed_first -> Bool,
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
    serveracl (category) {
        category -> Varchar,
        acl -> Jsonb,
    }
}

diesel::joinable!(bookmark -> player (player));
diesel::joinable!(publickey -> player (player));
diesel::joinable!(realm -> player (owner));
diesel::joinable!(realmannouncement -> realm (realm));
diesel::joinable!(realmcalendarsubscription -> player (player));
diesel::joinable!(realmcalendarsubscription -> realm (realm));
diesel::joinable!(realmchat -> realm (realm));
diesel::joinable!(remoteplayerchat -> player (player));
diesel::joinable!(remoteplayerlastread -> player (player));

diesel::allow_tables_to_appear_in_same_query!(
    announcement,
    authoidc,
    authotp,
    bannedpeers,
    bookmark,
    localplayerchat,
    localplayerlastread,
    player,
    publickey,
    realm,
    realmannouncement,
    realmcalendarsubscription,
    realmchat,
    realmtrain,
    remoteplayerchat,
    remoteplayerlastread,
    serveracl,
);

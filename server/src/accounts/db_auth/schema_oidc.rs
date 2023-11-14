// @generated automatically by Diesel CLI.

diesel::table! {
    auth_oidc (name) {
        name -> Text,
        subject -> Text,
        locked -> Bool,
        issuer -> Text,
    }
}

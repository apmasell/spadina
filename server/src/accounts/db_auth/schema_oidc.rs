// @generated automatically by Diesel CLI.

diesel::table! {
    authoidc (name) {
        name -> Text,
        subject -> Text,
        locked -> Bool,
        issuer -> Text,
    }
}

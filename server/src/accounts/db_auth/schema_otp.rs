// @generated automatically by Diesel CLI.

diesel::table! {
    authotp (name, code) {
        name -> Text,
        code -> Text,
        locked -> Bool,
    }
}

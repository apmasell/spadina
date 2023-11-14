// @generated automatically by Diesel CLI.

diesel::table! {
    auth_otp (name, code) {
        name -> Text,
        code -> Text,
        locked -> Bool,
    }
}

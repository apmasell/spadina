pub mod schema_oidc;
pub mod schema_otp;
pub const OIDC_MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!("oidc-migrations");
pub const OTP_MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!("otp-migrations");

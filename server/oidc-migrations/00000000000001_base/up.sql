CREATE TABLE auth_oidc (
    name text NOT NULL,
    subject text NOT NULL,
    locked boolean NOT NULL DEFAULT false,
    issuer text NOT NULL,
    PRIMARY KEY (name)
);

CREATE TABLE auth_otp (
    name text NOT NULL,
    code text NOT NULL,
    locked boolean NOT NULL DEFAULT false,
    PRIMARY KEY (name, code)
);

# Rustic-Typster

This is a typing practice game specifically for typing rust. The game scrapes crates.io for recently downloaded crates, finds their github repo, and pulls lines from any *.rs files.

![Screenshot](screenshots/rustic_typster_screenshot.png)

## Note about OpenSSL

Requires openssl libs to be installed. Depending on where your openssl installation is either change `.cargo/config.toml` or set environment variables.

server: RUST_LOG=info,burst=debug,burst_server=debug cargo run --bin burst -- burst.toml
gateway: ../Barbacane/target/release/barbacane serve --artifact burst-api.bca --listen 0.0.0.0:8080 --dev --allow-plaintext-upstream --log-format pretty
ui: cd ui && npm run dev

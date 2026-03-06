use burst_server::auth::hash_password;

fn main() {
    let hash = hash_password("burst123").expect("failed to hash");
    let id = uuid::Uuid::now_v7();
    println!(
        "INSERT INTO users (id, username, display_name, email, password_hash, role) VALUES ('{id}', 'alice', 'Alice Martin', 'alice@example.com', '{hash}', 'admin');"
    );
}

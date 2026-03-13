//! Seed the database with two test users and a shared channel.
//!
//! Usage:
//!   cargo run --example seed -- postgres://burst:burst@localhost:5432/burst
//!
//! Users are created with `external_id` set for Barbacane-based authentication.

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "postgres://burst:burst@localhost:5432/burst".into());

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;

    // Run migrations so the schema is up to date
    sqlx::migrate!("../../migrations").run(&pool).await?;

    let alice_id = Uuid::now_v7();
    let bob_id = Uuid::now_v7();
    let channel_id = Uuid::now_v7();

    // ── Users ──────────────────────────────────────────────────────────────────
    sqlx::query(
        "INSERT INTO users (id, username, display_name, email, external_id, role)
         VALUES ($1, 'alice', 'Alice Martin', 'alice@example.com', 'alice@example.com', 'admin')
         ON CONFLICT (email) DO NOTHING",
    )
    .bind(alice_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO users (id, username, display_name, email, external_id, role)
         VALUES ($1, 'bob', 'Bob Dupont', 'bob@example.com', 'bob@example.com', 'member')
         ON CONFLICT (email) DO NOTHING",
    )
    .bind(bob_id)
    .execute(&pool)
    .await?;

    // Look up the actual IDs in case the rows already existed
    let alice_id: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE email = 'alice@example.com'")
            .fetch_one(&pool)
            .await?;
    let bob_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 'bob@example.com'")
        .fetch_one(&pool)
        .await?;

    // ── Channel ────────────────────────────────────────────────────────────────
    sqlx::query(
        "INSERT INTO channels (id, kind, name, slug, created_by)
         VALUES ($1, 'public', 'general', 'general', $2)
         ON CONFLICT (slug) DO NOTHING",
    )
    .bind(channel_id)
    .bind(alice_id)
    .execute(&pool)
    .await?;

    let channel_id: Uuid = sqlx::query_scalar("SELECT id FROM channels WHERE slug = 'general'")
        .fetch_one(&pool)
        .await?;

    // ── Members ────────────────────────────────────────────────────────────────
    for (user_id, role) in [(alice_id, "owner"), (bob_id, "member")] {
        sqlx::query(
            "INSERT INTO channel_members (channel_id, user_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT (channel_id, user_id) DO NOTHING",
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(role)
        .execute(&pool)
        .await?;
    }

    println!("Seeded:");
    println!("  alice (admin, owner of #general) — id: {alice_id}");
    println!("  bob   (member of #general)       — id: {bob_id}");
    println!("  #general channel ready");
    println!();
    println!("Auth is handled by Barbacane basic-auth:");
    println!("  alice@example.com:alice  (admin)");
    println!("  bob@example.com:bob      (member)");

    Ok(())
}

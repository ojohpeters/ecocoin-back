use crate::models::{task::Task, user::UserInfo};
use once_cell::sync::Lazy;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

pub static DB_POOL: Lazy<PgPool> = Lazy::new(|| {
    let url = std::env::var("DATABASE_URL").expect("Missing DATABASE_URL");
    PgPoolOptions::new()
        .max_connections(5)
        .connect_lazy(&url)
        .expect("Failed to create DB pool")
});

pub async fn init_db() -> Result<(), sqlx::Error> {
    sqlx::migrate!()
        .run(&*DB_POOL)
        .await
        .map_err(|e| sqlx::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    Ok(())
}

// Create user if not exists
pub async fn create_user(wallet: &str) -> Result<Uuid, sqlx::Error> {
    let result = sqlx::query!(
        "INSERT INTO users (wallet_address) 
         VALUES ($1) 
         ON CONFLICT(wallet_address) DO NOTHING 
         RETURNING id",
        wallet
    )
    .fetch_optional(&*DB_POOL)
    .await?;

    if let Some(record) = result {
        Ok(record.id)
    } else {
        let existing = sqlx::query!("SELECT id FROM users WHERE wallet_address = $1", wallet)
            .fetch_one(&*DB_POOL)
            .await?;
        Ok(existing.id)
    }
}

// Lookup user by referral code (wallet or UUID)
pub async fn get_user_id_by_referral_code(code: &str) -> Result<Option<Uuid>, sqlx::Error> {
    if let Ok(uuid) = Uuid::parse_str(code) {
        let res = sqlx::query!("SELECT id FROM users WHERE referral_code = $1", uuid)
            .fetch_optional(&*DB_POOL)
            .await?;
        return Ok(res.map(|r| r.id));
    }

    let res = sqlx::query!("SELECT id FROM users WHERE wallet_address = $1", code)
        .fetch_optional(&*DB_POOL)
        .await?;
    Ok(res.map(|r| r.id))
}

// Set referral
pub async fn set_referrer(user_id: &Uuid, referrer_id: &Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE users SET referrer_id = $1 WHERE id = $2 AND referrer_id IS NULL",
        referrer_id,
        user_id
    )
    .execute(&*DB_POOL)
    .await?;
    Ok(())
}

// Add referral points to referrer
pub async fn add_referral_points(referrer_id: &Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE users SET total_points = total_points + 100 WHERE id = $1",
        referrer_id
    )
    .execute(&*DB_POOL)
    .await?;
    Ok(())
}

// Complete task
pub async fn complete_task(wallet: &str, task_id: Uuid) -> Result<(), sqlx::Error> {
    let user = sqlx::query!("SELECT id FROM users WHERE wallet_address = $1", wallet)
        .fetch_one(&*DB_POOL)
        .await?;

    // Check for duplicates
    let exists = sqlx::query!(
        "SELECT 1 as exists FROM completed_tasks WHERE user_id = $1 AND task_id = $2",
        user.id,
        task_id
    )
    .fetch_optional(&*DB_POOL)
    .await?;

    if exists.is_some() {
        return Err(sqlx::Error::RowNotFound);
    }

    let task = sqlx::query!("SELECT points FROM tasks WHERE id = $1", task_id)
        .fetch_one(&*DB_POOL)
        .await?;

    sqlx::query!(
        "INSERT INTO completed_tasks (user_id, task_id) VALUES ($1, $2)",
        user.id,
        task_id
    )
    .execute(&*DB_POOL)
    .await?;

    sqlx::query!(
        "UPDATE users SET total_points = total_points + $1 WHERE id = $2",
        task.points,
        user.id
    )
    .execute(&*DB_POOL)
    .await?;

    Ok(())
}

// Fetch user points + completed tasks + referral count
pub async fn get_user_info(wallet: &str) -> Result<UserInfo, sqlx::Error> {
    let user = sqlx::query!(
        "SELECT id, total_points FROM users WHERE wallet_address = $1",
        wallet
    )
    .fetch_one(&*DB_POOL)
    .await?;

    let completed_tasks = sqlx::query!(
        "SELECT task_id FROM completed_tasks WHERE user_id = $1",
        user.id
    )
    .fetch_all(&*DB_POOL)
    .await?
    .into_iter()
    .map(|r| r.task_id)
    .collect();

    let referrals = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE referrer_id = $1",
        user.id
    )
    .fetch_one(&*DB_POOL)
    .await?
    .count
    .unwrap_or(0);

    Ok(UserInfo {
        wallet: wallet.to_string(),
        total_points: user.total_points.unwrap_or(0),
        tasks_completed: completed_tasks,
        referrals,
    })
}

// Get all tasks
pub async fn get_all_tasks() -> Result<Vec<Task>, sqlx::Error> {
    let records = sqlx::query_as!(Task, "SELECT id, name, points, description FROM tasks")
        .fetch_all(&*DB_POOL)
        .await?;
    Ok(records)
}

pub async fn get_wallet_count() -> Result<i64, sqlx::Error> {
    let row = sqlx::query!("SELECT COUNT(*) as count FROM users")
        .fetch_one(&*DB_POOL)
        .await?;

    Ok(row.count.unwrap_or(0))
}

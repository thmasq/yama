use super::{hex_to_binary_vector, SaveHashRequest};
use crate::AppState;
use actix_web::{web, HttpResponse, Responder};
use sqlx::{Pool, Postgres};

#[allow(clippy::significant_drop_tightening)]
pub async fn save_hash(
    data: web::Data<AppState>,
    req: web::Json<SaveHashRequest>,
    db_pool: web::Data<Pool<Postgres>>,
) -> impl Responder {
    let SaveHashRequest {
        hash,
        guild_id,
        channel_id,
        message_id,
    } = req.into_inner();
    let hash_vec = hex_to_binary_vector(&hash);

    let hnsw = data.hnsw.lock().await;

    let existing_record = sqlx::query!(
        "SELECT id FROM image_hashes WHERE hash = $1 AND guild_id = $2",
        hash,
        guild_id
    )
    .fetch_optional(db_pool.get_ref())
    .await;

    match existing_record {
        Ok(Some(record)) => {
            HttpResponse::Ok().json(format!("Hash already exists with ID {}", record.id))
        }
        Ok(None) => {
            match sqlx::query!(
                "INSERT INTO image_hashes (hash, guild_id, channel_id, message_id, timestamp)
                 VALUES ($1, $2, $3, $4, NOW())
                 RETURNING id",
                hash,
                guild_id,
                channel_id,
                message_id
            )
            .fetch_one(db_pool.get_ref())
            .await
            {
                Ok(inserted) => {
                    hnsw.insert((&hash_vec, inserted.id as usize));
                    HttpResponse::Created().json(format!("Hash saved with ID {}", inserted.id))
                }
                Err(err) => {
                    eprintln!("Failed to insert hash: {err:?}");
                    HttpResponse::InternalServerError().json("Failed to save hash")
                }
            }
        }
        Err(err) => {
            eprintln!("Error querying the database: {err:?}");
            HttpResponse::InternalServerError().json("Failed to query hash")
        }
    }
}

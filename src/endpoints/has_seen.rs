use super::hex_to_binary_vector;
use crate::{AppState, SIMILARITY_THRESHOLD};
use actix_web::{web, HttpResponse, Responder};
use sqlx::{Pool, Postgres};

pub async fn has_seen(
    data: web::Data<AppState>,
    hash: web::Path<String>,
    db_pool: web::Data<Pool<Postgres>>,
) -> impl Responder {
    let hash_str = hash.into_inner();
    let hash_vec = hex_to_binary_vector(&hash_str);

    let neighbors = data.hnsw.lock().await.search(&hash_vec, 10, 10);

    let mut results = Vec::new();

    for neighbor in neighbors {
        if neighbor.distance < SIMILARITY_THRESHOLD {
            if let Ok(records) = sqlx::query!(
                "SELECT hash, guild_id, channel_id, message_id, timestamp 
                 FROM image_hashes 
                 WHERE hash = $1",
                neighbor.d_id as i64
            )
            .fetch_all(db_pool.get_ref())
            .await
            {
                for record in records {
                    results.push(serde_json::json!({
                        "hash": record.hash,
                        "guild_id": record.guild_id,
                        "channel_id": record.channel_id,
                        "message_id": record.message_id,
                        "timestamp": record.timestamp
                    }));
                }
            }
        }
    }

    if results.is_empty() {
        HttpResponse::Ok().json(Vec::<serde_json::Value>::new())
    } else {
        HttpResponse::Ok().json(results)
    }
}

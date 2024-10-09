use super::{hex_to_binary_vector, SaveHashRequest};
use crate::{AppState, SIMILARITY_THRESHOLD};
use actix_web::{web, HttpResponse, Responder};
use sqlx::{Pool, Postgres};

pub async fn check_and_save_hash(
    req: web::Json<SaveHashRequest>,
    app_state: web::Data<AppState>,
    db_pool: web::Data<Pool<Postgres>>,
) -> impl Responder {
    let SaveHashRequest {
        hash,
        guild_id,
        channel_id,
        message_id,
    } = req.into_inner();
    let hash_vec = hex_to_binary_vector(&hash);

    let hnsw = app_state.hnsw.lock().await;
    let neighbors = hnsw.search(&hash_vec, 10, 10);

    let mut results = Vec::new();
    for neighbor in neighbors {
        if neighbor.distance < SIMILARITY_THRESHOLD {
            match sqlx::query!(
                "SELECT hash, guild_id, channel_id, message_id, timestamp 
                 FROM image_hashes 
                 WHERE hash = $1",
                neighbor.d_id as i64
            )
            .fetch_all(db_pool.get_ref())
            .await
            {
                Ok(records) => {
                    for record in records {
                        results.push(serde_json::json!({
                            "hash": record.hash,
                            "guild_id": record.guild_id,
                            "channel_id": record.channel_id,
                            "message_id": record.message_id,
                            "timestamp": record.timestamp,
                        }));
                    }
                }
                Err(_) => {
                    return HttpResponse::InternalServerError().json("Failed to query database");
                }
            }
        }
    }

    if !results.is_empty() {
        return HttpResponse::Ok().json(results);
    }

    drop(hnsw);

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
        Ok(inserted_record) => {
            app_state
                .hnsw
                .lock()
                .await
                .insert((&hash_vec, inserted_record.id as usize));

            HttpResponse::Created().json(serde_json::json!({
                "message": "Hash saved successfully",
                "id": inserted_record.id,
            }))
        }
        Err(_) => HttpResponse::InternalServerError().json("Failed to save hash"),
    }
}

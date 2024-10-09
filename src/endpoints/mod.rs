use actix_web::web;
use serde::Deserialize;

pub mod check_and_save_hash;
pub mod has_seen;
pub mod save_hash;

use check_and_save_hash::check_and_save_hash;
use has_seen::has_seen;
use save_hash::save_hash;

#[derive(Deserialize)]
pub struct SaveHashRequest {
    hash: String,
    guild_id: i64,
    channel_id: i64,
    message_id: i64,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/has_seen/{hash}").route(web::get().to(has_seen)))
        .service(web::resource("/check_and_save_hash").route(web::get().to(check_and_save_hash)))
        .service(web::resource("/save_hash").route(web::post().to(save_hash)));
}

fn hex_to_binary_vector(hash: &str) -> Vec<u8> {
    hash.chars()
        .flat_map(|c| {
            let byte = u8::from_str_radix(&c.to_string(), 16).unwrap_or(0);
            (0..4)
                .map(move |i| (byte >> (3 - i)) & 1)
                .collect::<Vec<u8>>()
        })
        .collect()
}

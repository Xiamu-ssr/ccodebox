use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../frontend/out"]
struct Assets;

pub async fn serve_frontend(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // 1. Exact match for static file
    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data,
        )
            .into_response();
    }

    // 2. Try path/index.html (Next.js export generates e.g. settings/index.html)
    let index_path = if path.is_empty() {
        "index.html".to_string()
    } else {
        format!("{path}/index.html")
    };
    if let Some(file) = Assets::get(&index_path) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            file.data,
        )
            .into_response();
    }

    // 3. Try path.html (Next.js may also export settings.html)
    let html_path = format!("{path}.html");
    if let Some(file) = Assets::get(&html_path) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            file.data,
        )
            .into_response();
    }

    // 4. SPA fallback — return root index.html for client-side routing
    match Assets::get("index.html") {
        Some(file) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            file.data,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Frontend not found").into_response(),
    }
}

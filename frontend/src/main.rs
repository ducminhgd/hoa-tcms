//! HOA TCMS — Leptos frontend (SSR server entry point).

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use hoa_tcms_frontend::app::App;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use tower_http::services::ServeDir;

    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, || view! { <App /> })
        .fallback_service(ServeDir::new(&*leptos_options.site_root))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("frontend listening on http://{addr}");
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}

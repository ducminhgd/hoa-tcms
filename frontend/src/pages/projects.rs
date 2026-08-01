use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use crate::API_BASE;
#[cfg(feature = "hydrate")]
use gloo_net::http::Request;

#[component]
pub fn ProjectsPage() -> impl IntoView {
    // Explicit element type: the fetch effect that populates this signal is
    // compiled out of the SSR build (browser-only), so inference alone would
    // leave `Vec::new()` untyped. The write-handles are only used by the
    // hydrate-gated effect below, hence the `allow(unused_variables)` in the
    // SSR (non-hydrate) build.
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (projects, set_projects) = signal(Vec::<serde_json::Value>::new());
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (loading, set_loading) = signal(true);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (error, set_error) = signal(String::new());

    // Fetch only on the client (hydrate build): `gloo_net` + `spawn_local`
    // are browser-only APIs and panic during SSR (`spawn_local` called from
    // outside a LocalSet). The server render shows the "Loading..." state;
    // the WASM bundle performs the actual request after hydration.
    #[cfg(feature = "hydrate")]
    Effect::new(move || {
        leptos::task::spawn_local(async move {
            let resp = Request::get(&format!("{}/api/v1/projects", API_BASE))
                .credentials(web_sys::RequestCredentials::Include)
                .header("Accept", "application/json")
                .send()
                .await;

            set_loading.set(false);

            match resp {
                Ok(r) if r.ok() => {
                    let body = r.text().await.unwrap_or_default();
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body)
                        && let Some(data) = parsed.get("data").and_then(|d| d.as_array())
                    {
                        set_projects.set(data.to_vec());
                    }
                }
                Ok(r) if r.status() == 401 => {
                    set_error.set("Not authenticated. Please login first.".into());
                }
                Ok(r) => {
                    set_error.set(format!("Failed to load projects (status {})", r.status()));
                }
                Err(e) => {
                    set_error.set(format!("Network error: {e}"));
                }
            }
        });
    });

    view! {
        <div class="card">
            <h1>Projects</h1>
        </div>

        {move || {
            if loading.get() {
                view! { <p>Loading...</p> }.into_any()
            } else if !error.get().is_empty() {
                view! { <p class="error">{error.get()}</p> }.into_any()
            } else {
                projects.get().into_iter().map(|p| {
                    let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("-").to_string();
                    let desc = p.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                    view! {
                        <div class="card">
                            <h3>{name}</h3>
                            <p>{desc}</p>
                        </div>
                    }
                }).collect::<Vec<_>>().into_any()
            }
        }}
    }
}

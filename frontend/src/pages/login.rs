use gloo_net::http::Request;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::API_BASE;

#[component]
pub fn LoginPage() -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let navigate = use_navigate();

    let login = move |_| {
        let u = username.get();
        let p = password.get();

        if u.is_empty() || p.is_empty() {
            set_error.set("Username and password are required.".into());
            return;
        }

        let nav = navigate.clone();
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({
                "username_or_email": u,
                "password": p,
            });

            let req = Request::post(&format!("{}/api/v1/auth/login", API_BASE))
                .credentials(web_sys::RequestCredentials::Include)
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&body).unwrap())
                .unwrap();

            match req.send().await {
                Ok(r) if r.ok() => {
                    set_error.set(String::new());
                    nav("/projects", Default::default());
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    set_error.set(format!("Login failed ({status}): {text}"));
                }
                Err(e) => {
                    set_error.set(format!("Network error: {e}"));
                }
            }
        });
    };

    view! {
        <div class="card" style="max-width:400px;margin:2rem auto">
            <h1>Login</h1>
            <div class="form-group">
                <label class="label" for="username">Username or Email</label>
                <input class="input" id="username" type="text" placeholder="admin"
                    on:input=move |ev| set_username.set(event_target_value(&ev)) />
            </div>
            <div class="form-group">
                <label class="label" for="password">Password</label>
                <input class="input" id="password" type="password"
                    on:input=move |ev| set_password.set(event_target_value(&ev)) />
            </div>
            <button class="btn btn-primary" on:click=login>Login</button>
            {move || {
                if !error.get().is_empty() {
                    view! { <p class="error">{error.get()}</p> }.into_any()
                } else {
                    ().into_any()
                }
            }}
        </div>
    }
}

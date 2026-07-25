//! Login page.

use leptos::prelude::*;
use leptos_router::*;
use crate::api;

#[component]
pub fn LoginPage() -> impl IntoView {
    let (username, set_username) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (error, set_error) = create_signal(String::new());
    let navigate = use_navigate();
    let set_auth: SignalSetter<bool> = use_context().unwrap();

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let u = username.get();
        let p = password.get();
        set_error.set(String::new());
        spawn_local(async move {
            let req = api::LoginRequest {
                username_or_email: u,
                password: p,
            };
            match api::login(&req).await {
                Ok(()) => {
                    set_auth.set(true);
                    navigate("/", Default::default());
                }
                Err(e) => set_error.set(e),
            }
        });
    };

    view! {
        <div class="login-page">
            <form class="login-form" on:submit=on_submit>
                <h1>"HOA TCMS"</h1>
                <label>
                    "Username or Email"
                    <input type="text" required
                        on:input=move |ev| set_username.set(event_target_value(&ev))
                        prop:value=username />
                </label>
                <label>
                    "Password"
                    <input type="password" required
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                        prop:value=password />
                </label>
                <button type="submit">"Login"</button>
                <Show when=move || !error.get().is_empty()>
                    <p class="error">{error}</p>
                </Show>
            </form>
        </div>
    }
}

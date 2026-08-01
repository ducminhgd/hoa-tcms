use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::components::*;
use leptos_router::path;

use crate::pages;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Html attr:lang="en" />
        <Title text="HOA TCMS" />

        <Router>
            <Nav />
            <main class="container">
                <Routes fallback=|| view! { <NotFound /> }>
                    <Route path=path!("/") view=pages::home::HomePage />
                    <Route path=path!("/login") view=pages::login::LoginPage />
                    <Route path=path!("/projects") view=pages::projects::ProjectsPage />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Nav() -> impl IntoView {
    view! {
        <nav class="nav container">
            <A href="/">Home</A>
            <A href="/projects">Projects</A>
            <A href="/login">Login</A>
        </nav>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div class="card">
            <h1>404</h1>
            <p>Page not found.</p>
            <A href="/">Go home</A>
        </div>
    }
}

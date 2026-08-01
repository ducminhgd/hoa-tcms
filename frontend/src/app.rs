use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::components::*;
use leptos_router::path;

use crate::pages;

/// Renders the full HTML document shell around the app.
///
/// This is required for SSR: `leptos_meta` (e.g. `<Title>`) injects tags into
/// the streamed HTML at the `</head>` marker, so the shell must emit the
/// `<html>`/`<head>`/`<body>` skeleton. Without it, SSR panics with
/// "you are using leptos_meta without a </head> tag".
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/hoa-frontend.css"/>
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

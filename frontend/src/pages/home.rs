use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="card">
            <h1>HOA TCMS</h1>
            <p>Test Case Management System</p>
        </div>
        <div class="card">
            <h2>Quick Links</h2>
            <ul>
                <li><a href="/projects">Projects</a></li>
                <li><a href="/login">Login</a></li>
            </ul>
        </div>
    }
}

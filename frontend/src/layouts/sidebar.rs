//! Sidebar navigation component.

use leptos::prelude::*;
use leptos_router::*;

#[component]
pub fn Sidebar() -> impl IntoView {
    view! {
        <nav class="sidebar">
            <div class="sidebar-header">
                <h2>"HOA TCMS"</h2>
            </div>
            <div class="sidebar-section">
                <h3>"🔐 IAM"</h3>
                <a href="/users">"Users"</a>
                <a href="/groups">"Groups"</a>
                <a href="/roles">"Roles"</a>
                <a href="/permissions">"Permissions"</a>
            </div>
            <div class="sidebar-section">
                <h3>"📁 Projects"</h3>
                <a href="/projects">"Projects"</a>
            </div>
            <div class="sidebar-section">
                <h3>"🧪 Test Management"</h3>
                <a href="/test-plans">"Test Plans"</a>
                <a href="/test-cases">"Test Cases"</a>
                <a href="/test-runs">"Test Runs"</a>
                <a href="/test-executions">"Test Executions"</a>
            </div>
            <div class="sidebar-footer">
                <A href="/login">"Logout"</A>
            </div>
        </nav>
    }
}

//! Application shell — router, layout, and global state.

use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::*;

use crate::layouts::sidebar::Sidebar;
use crate::pages::*;

/// Main application component with router.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let (is_authenticated, set_auth) = create_signal(false);
    let (current_project, set_project) = create_signal::<(i64, String)>((0, String::new()));

    provide_context(is_authenticated);
    provide_context(set_auth);
    provide_context(current_project);
    provide_context(set_project);

    view! {
        <Stylesheet id="leptos" href="/static/styles.css" />
        <Router>
            <main class="app-layout">
                <Show when=move || is_authenticated.get()>
                    <Sidebar />
                </Show>
                <div class="main-content">
                    <Routes fallback=|| "Page not found">
                        <Route path="/login" view=LoginPage />
                        <Route path="/" view=Dashboard />
                        <Route path="/projects" view=ProjectList />
                        <Route path="/projects/new" view=ProjectForm />
                        <Route path="/projects/:id" view=ProjectDetail />
                        <Route path="/test-plans" view=TestPlanList />
                        <Route path="/test-plans/new" view=TestPlanForm />
                        <Route path="/test-plans/:id" view=TestPlanDetail />
                        <Route path="/test-runs" view=TestRunList />
                        <Route path="/test-runs/new" view=TestRunForm />
                        <Route path="/test-runs/:id" view=TestRunDetail />
                        <Route path="/test-executions" view=TestExecutionList />
                        <Route path="/test-executions/new" view=TestExecutionForm />
                        <Route path="/test-executions/:id" view=TestExecutionDetail />
                        <Route path="/test-cases" view=TestCaseList />
                        <Route path="/test-cases/new" view=TestCaseForm />
                        <Route path="/users" view=UserList />
                        <Route path="/groups" view=|| view! { <h1>"Groups"</h1> } />
                        <Route path="/roles" view=|| view! { <h1>"Roles"</h1> } />
                        <Route path="/permissions" view=|| view! { <h1>"Permissions"</h1> } />
                    </Routes>
                </div>
            </main>
        </Router>
    }
}

/// Dashboard / home page.
#[component]
fn Dashboard() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"HOA TCMS"</h1>
            <p>"Test Case Management System — select a section from the sidebar."</p>
        </div>
    }
}

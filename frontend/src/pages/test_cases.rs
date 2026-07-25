use leptos::prelude::*;
use leptos_router::*;

#[component]
pub fn TestCaseList() -> impl IntoView {
    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Test Cases"</h1>
                <A href="/test-cases/new" class="btn btn-primary">"+ Add New"</A>
            </div>
            <p>"Test case management — list view coming soon."</p>
        </div>
    }
}

#[component]
pub fn TestCaseForm() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"Create Test Case"</h1>
            <p>"Test case creation coming soon."</p>
        </div>
    }
}

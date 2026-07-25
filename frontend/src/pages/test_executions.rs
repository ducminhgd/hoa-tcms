use leptos::prelude::*;
use leptos_router::*;
use crate::api;
use crate::components::data_table::DataTable;
use crate::components::pagination::Pagination;

#[component]
pub fn TestExecutionList() -> impl IntoView {
    let (page, set_page) = create_signal(1u32);
    let (executions, set_executions) = create_signal(Vec::new());
    let (total, set_total) = create_signal(0u64);

    let load = move || {
        let p = page.get();
        spawn_local(async move {
            match api::list_test_executions(p, 25).await {
                Ok(resp) => { set_executions.set(resp.data); set_total.set(resp.meta.total); }
                Err(e) => log::error!("{}", e),
            }
        });
    };
    create_effect(move |_| load());

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Test Executions"</h1>
                <A href="/test-executions/new" class="btn btn-primary">"+ Add New"</A>
            </div>
            <DataTable
                columns=vec!["ID", "Name", "Testers", "Updated"]
                rows=move || executions.get().into_iter().map(|e| vec![
                    e.id.to_string(),
                    format!("<a href='/test-executions/{}'>{}</a>", e.id, e.name),
                    e.tester_count.to_string(), e.updated_at,
                ]).collect()
            />
            <Pagination page total=move || total.get() limit=25
                on_page=move |p| { set_page.set(p); load(); } />
        </div>
    }
}

#[component]
pub fn TestExecutionForm() -> impl IntoView {
    let (name, set_name) = create_signal(String::new());
    let (run_id, set_run_id) = create_signal(String::new());
    let navigate = use_navigate();
    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let body = serde_json::json!({
                "name": name.get(),
                "test_run_id": run_id.get().parse::<i64>().unwrap_or(0),
            });
            let _ = api::post(
                &format!("/api/v1/test-executions"),
                &body,
            ).await;
            navigate("/test-executions", Default::default());
        });
    };
    view! {
        <div class="page">
            <h1>"Create Test Execution"</h1>
            <form class="form" on:submit=on_submit>
                <label>"Name" <input type="text" required
                    on:input=move |ev| set_name.set(event_target_value(&ev)) /></label>
                <label>"Test Run ID" <input type="number"
                    on:input=move |ev| set_run_id.set(event_target_value(&ev)) /></label>
                <button type="submit" class="btn btn-primary">"Create"</button>
            </form>
        </div>
    }
}

#[component]
pub fn TestExecutionDetail() -> impl IntoView {
    let params = use_params_map();
    view! { <div class="page"><h1>"Test Execution " {move || params.get().get("id").cloned().unwrap_or_default()}</h1></div> }
}

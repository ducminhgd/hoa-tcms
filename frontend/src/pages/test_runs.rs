use leptos::prelude::*;
use leptos_router::*;
use crate::api;
use crate::components::data_table::DataTable;
use crate::components::pagination::Pagination;

#[component]
pub fn TestRunList() -> impl IntoView {
    let (page, set_page) = create_signal(1u32);
    let (runs, set_runs) = create_signal(Vec::new());
    let (total, set_total) = create_signal(0u64);
    let (project_id, _) = create_signal(1i64);

    let load = move || {
        let p = page.get();
        let pid = project_id.get();
        spawn_local(async move {
            match api::list_test_runs(pid, p, 25).await {
                Ok(resp) => { set_runs.set(resp.data); set_total.set(resp.meta.total); }
                Err(e) => log::error!("{}", e),
            }
        });
    };
    create_effect(move |_| load());

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Test Runs"</h1>
                <A href="/test-runs/new" class="btn btn-primary">"+ Add New"</A>
            </div>
            <DataTable
                columns=vec!["ID", "Summary", "Cases", "Updated"]
                rows=move || runs.get().into_iter().map(|r| vec![
                    r.id.to_string(),
                    format!("<a href='/test-runs/{}'>{}</a>", r.id, r.summary),
                    r.case_count.to_string(), r.updated_at,
                ]).collect()
            />
            <Pagination page total=move || total.get() limit=25
                on_page=move |p| { set_page.set(p); load(); } />
        </div>
    }
}

#[component]
pub fn TestRunForm() -> impl IntoView {
    let (summary, set_summary) = create_signal(String::new());
    let navigate = use_navigate();
    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let s = summary.get();
        spawn_local(async move {
            let body = api::CreateTestRun {
                summary: s, report_to: None, default_tester: None,
                plan_id: None, version: None, notes: None,
                planned_start_date: None, planned_end_date: None,
            };
            match api::create_test_run(1, &body).await {
                Ok(_) => navigate("/test-runs", Default::default()),
                Err(e) => log::error!("{}", e),
            }
        });
    };
    view! {
        <div class="page">
            <h1>"Create Test Run"</h1>
            <form class="form" on:submit=on_submit>
                <label>"Summary" <input type="text" required
                    on:input=move |ev| set_summary.set(event_target_value(&ev)) /></label>
                <button type="submit" class="btn btn-primary">"Create"</button>
            </form>
        </div>
    }
}

#[component]
pub fn TestRunDetail() -> impl IntoView {
    let params = use_params_map();
    view! { <div class="page"><h1>"Test Run " {move || params.get().get("id").cloned().unwrap_or_default()}</h1></div> }
}

//! Test Plan pages — list, create, detail.

use leptos::prelude::*;
use leptos_router::*;
use crate::api;
use crate::components::data_table::DataTable;
use crate::components::pagination::Pagination;

#[component]
pub fn TestPlanList() -> impl IntoView {
    let (page, set_page) = create_signal(1u32);
    let (plans, set_plans) = create_signal(Vec::new());
    let (total, set_total) = create_signal(0u64);

    let load = move || {
        let p = page.get();
        spawn_local(async move {
            match api::list_test_plans(p, 25).await {
                Ok(resp) => { set_plans.set(resp.data); set_total.set(resp.meta.total); }
                Err(e) => log::error!("{}", e),
            }
        });
    };

    create_effect(move |_| load());

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Test Plans"</h1>
                <A href="/test-plans/new" class="btn btn-primary">"+ Add New"</A>
            </div>
            <DataTable
                columns=vec!["ID", "Name", "Version", "Status", "Updated"]
                rows=move || plans.get().into_iter().map(|p| vec![
                    p.id.to_string(),
                    format!("<a href='/test-plans/{}'>{}</a>", p.id, p.name),
                    p.version, p.status, p.updated_at,
                ]).collect()
            />
            <Pagination page total=move || total.get() limit=25
                on_page=move |p| { set_page.set(p); load(); } />
        </div>
    }
}

#[component]
pub fn TestPlanForm() -> impl IntoView {
    let (name, set_name) = create_signal(String::new());
    let (types, set_types) = create_signal(String::new());
    let navigate = use_navigate();

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let n = name.get();
        let t: Vec<String> = types.get().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        spawn_local(async move {
            let body = api::CreateTestPlan {
                name: n, project_ids: vec![1], types: t,
                version: "1.0".into(), description: None,
            };
            match api::create_test_plan(&body).await {
                Ok(_) => navigate("/test-plans", Default::default()),
                Err(e) => log::error!("{}", e),
            }
        });
    };

    view! {
        <div class="page">
            <h1>"Create Test Plan"</h1>
            <form class="form" on:submit=on_submit>
                <label>"Name" <input type="text" required
                    on:input=move |ev| set_name.set(event_target_value(&ev)) /></label>
                <label>"Types (comma-separated)" <input type="text"
                    placeholder="REGRESSION, API" on:input=move |ev| set_types.set(event_target_value(&ev)) /></label>
                <button type="submit" class="btn btn-primary">"Create"</button>
            </form>
        </div>
    }
}

#[component]
pub fn TestPlanDetail() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.get().get("id").cloned().unwrap_or_default();
    view! { <div class="page"><h1>"Test Plan " {id}</h1></div> }
}

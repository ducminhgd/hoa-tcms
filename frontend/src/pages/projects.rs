//! Project pages — list, create, detail.

use leptos::prelude::*;
use leptos_router::*;
use crate::api;
use crate::components::data_table::DataTable;
use crate::components::pagination::Pagination;

#[component]
pub fn ProjectList() -> impl IntoView {
    let (page, set_page) = create_signal(1u32);
    let (projects, set_projects) = create_signal(Vec::new());
    let (total, set_total) = create_signal(0u64);

    let load = move || {
        let p = page.get();
        spawn_local(async move {
            match api::list_projects(p, 25).await {
                Ok(resp) => {
                    set_projects.set(resp.data);
                    set_total.set(resp.meta.total);
                }
                Err(e) => log::error!("{}", e),
            }
        });
    };

    create_effect(move |_| load());

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Projects"</h1>
                <A href="/projects/new" class="btn btn-primary">"+ Add New"</A>
            </div>
            <DataTable
                columns=vec!["ID", "Name", "Status", "Created"]
                rows=move || {
                    projects.get().into_iter().map(|p| {
                        vec![
                            p.id.to_string(),
                            format!("<a href='/projects/{}'>{}</a>", p.id, p.name),
                            p.status,
                            p.created_at,
                        ]
                    }).collect()
                }
            />
            <Pagination
                page
                total=move || total.get()
                limit=25
                on_page=move |p| { set_page.set(p); load(); }
            />
        </div>
    }
}

#[component]
pub fn ProjectForm() -> impl IntoView {
    let (name, set_name) = create_signal(String::new());
    let (desc, set_desc) = create_signal(String::new());
    let navigate = use_navigate();

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let n = name.get();
        let d = desc.get();
        spawn_local(async move {
            let body = api::CreateProject {
                name: n,
                description: if d.is_empty() { None } else { Some(d) },
                status: "ACTIVE".into(),
            };
            match api::create_project(&body).await {
                Ok(_) => navigate("/projects", Default::default()),
                Err(e) => log::error!("{}", e),
            }
        });
    };

    view! {
        <div class="page">
            <h1>"Create Project"</h1>
            <form class="form" on:submit=on_submit>
                <label>"Name" <input type="text" required
                    on:input=move |ev| set_name.set(event_target_value(&ev)) /></label>
                <label>"Description" <textarea
                    on:input=move |ev| set_desc.set(event_target_value(&ev)) /></label>
                <button type="submit" class="btn btn-primary">"Create"</button>
            </form>
        </div>
    }
}

#[component]
pub fn ProjectDetail() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.get().get("id").cloned().unwrap_or_default();

    view! {
        <div class="page">
            <h1>"Project " {id}</h1>
            <p>"Detail view coming soon."</p>
        </div>
    }
}

use leptos::prelude::*;
use crate::api;
use crate::components::data_table::DataTable;
use crate::components::pagination::Pagination;

#[component]
pub fn UserList() -> impl IntoView {
    let (page, set_page) = create_signal(1u32);
    let (users, set_users) = create_signal(Vec::new());
    let (total, set_total) = create_signal(0u64);

    let load = move || {
        let p = page.get();
        spawn_local(async move {
            match api::list_users(p, 25).await {
                Ok(resp) => { set_users.set(resp.data); set_total.set(resp.meta.total); }
                Err(e) => log::error!("{}", e),
            }
        });
    };
    create_effect(move |_| load());

    view! {
        <div class="page">
            <h1>"Users"</h1>
            <DataTable
                columns=vec!["ID", "Username", "Email", "Full Name", "Status"]
                rows=move || users.get().into_iter().map(|u| vec![
                    u.id.to_string(), u.username, u.email, u.fullname, u.status,
                ]).collect()
            />
            <Pagination page total=move || total.get() limit=25
                on_page=move |p| { set_page.set(p); load(); } />
        </div>
    }
}

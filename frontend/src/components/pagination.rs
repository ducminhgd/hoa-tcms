//! Pagination component with configurable page sizes.

use leptos::prelude::*;

/// Server-side pagination controls.
///
/// Displays page info and Previous/Next buttons. The `on_page` callback is
/// called with the new page number when the user navigates.
#[component]
pub fn Pagination(
    page: Signal<u32>,
    total: Signal<u64>,
    limit: u32,
    on_page: impl Fn(u32) + 'static,
) -> impl IntoView {
    let total_pages = move || {
        let t = total.get();
        if t == 0 { 1u32 } else { ((t + limit as u64 - 1) / limit as u64) as u32 }
    };

    view! {
        <div class="pagination">
            <span class="pagination-info">
                "Page " {page} " of " {total_pages} " (" {total} " total)"
            </span>
            <button
                class="btn btn-sm"
                disabled=move || page.get() <= 1
                on:click=move |_| {
                    let p = page.get();
                    if p > 1 { on_page(p - 1); }
                }
            >
                "← Previous"
            </button>
            <button
                class="btn btn-sm"
                disabled=move || page.get() >= total_pages()
                on:click=move |_| {
                    let p = page.get();
                    let tp = total_pages();
                    if p < tp { on_page(p + 1); }
                }
            >
                "Next →"
            </button>
        </div>
    }
}

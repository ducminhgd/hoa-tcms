//! Reusable data table component with checkbox column and action icons.

use leptos::prelude::*;

/// A data table with configurable columns and rows.
///
/// Each row is a `Vec<String>` where the first column typically contains a
/// checkbox, and the content can include inline HTML (e.g. links).
#[component]
pub fn DataTable(
    columns: Vec<&'static str>,
    rows: Signal<Vec<Vec<String>>>,
) -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th><input type="checkbox" /></th>
                    {columns.into_iter().map(|c| view! { <th>{c}</th> }).collect::<Vec<_>>()}
                    <th>"Actions"</th>
                </tr>
            </thead>
            <tbody>
                <For
                    each=rows
                    key=|row| row.first().cloned().unwrap_or_default()
                    children=move |row| {
                        let mut cells = row.clone();
                        let id = if cells.is_empty() { String::new() } else { cells.remove(0) };
                        view! {
                            <tr>
                                <td><input type="checkbox" value=id.clone() /></td>
                                {cells.into_iter().map(|c| view! {
                                    <td inner_html=c />
                                }).collect::<Vec<_>>()}
                                <td class="actions">
                                    <button class="btn-icon" title="Edit">"✏️"</button>
                                    <button class="btn-icon" title="Delete">"🗑️"</button>
                                </td>
                            </tr>
                        }
                    }
                />
            </tbody>
        </table>
    }
}

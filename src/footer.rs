use maud::{html, Markup, Render};
use serde::{Deserialize, Serialize};

use crate::{filter::Filter, state::State};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Footer {
    pub current_filter: Filter,
    pub num_active: usize,
    pub num_completed: usize,
    pub oob: bool,
}

impl From<&State> for Footer {
    fn from(state: &State) -> Self {
        Self {
            num_active: state.todos.iter().filter(|todo| !todo.completed).count(),
            num_completed: state.todos.iter().filter(|todo| todo.completed).count(),
            current_filter: state.filter.clone(),
            oob: true,
        }
    }
}

impl Render for Footer {
    fn render(&self) -> Markup {
        html! { footer.footer #footer hx-swap-oob=[self.oob.then(|| "true")]
            hx-target="footer.footer" hx-swap="outerHTML" {
            span.todo-count {
                strong { (self.num_active) }
                " item" @if self.num_active != 1 { "s" } " left"
            }

            ul.filters hx-include="next input" {
                @for filter in [Filter::All, Filter::Active, Filter::Completed] { li {
                    a.selected[self.current_filter == filter] hx-post=("/select") { (filter.to_string()) }
                    input type="hidden" name="filter" value=(filter.to_string());
                } }
            }

            @if self.num_completed > 0 {
                button.clear-completed hx-post="/clear-completed" { "Clear completed" }
            }
        } }
    }
}

use maud::{html, Markup, Render};
use serde::{Deserialize, Serialize};

use crate::{filter::Filter, footer::Footer, state::State};

#[derive(Debug, Deserialize, Serialize)]
pub enum TodoPlaceholder {
    Extend,
    FullPayload,
}

impl Render for TodoPlaceholder {
    fn render(&self) -> Markup {
        html! { @match self {
            TodoPlaceholder::Extend => input type="hidden" name="next-todo" value="Extend";,
            TodoPlaceholder::FullPayload => input #todo-list hx-swap-oob="true"
                type="hidden" name="next-todo" value="FullPayload";
        } }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Todo {
    pub completed: bool,
    pub description: String,
    pub id: u64,
}

impl Render for Todo {
    fn render(&self) -> Markup {
        html! {
            li.completed[self.completed] #{"todo-" (self.id)}
                x-data={ r#"{"editing":false,"description":""# (self.description) r#""}"# }
                x-bind:class=r#"editing && "editing""#
                x-on:dblclick="editing = !editing; $nextTick(() => $refs['edit-todo-input'].focus())"
                hx-swap="outerHTML" hx-target={"#todo-" (self.id)} {
                    div.view x-show="!editing" {
                        input.toggle type="checkbox" checked[self.completed]
                            hx-patch={"/todo/" (self.id)} hx-include="next input[name='completed']";
                        label x-text="description" { (self.description) }
                        button.destroy hx-delete={"/todo/" (self.id)} { }
                    }
                    input type="hidden" name="completed" value=(self.completed);

                    template x-if="editing" { div.input-container {
                        input.edit #edit-todo-input x-ref="edit-todo-input"
                            hx-patch={"/todo/" (self.id)} name="desc" x-model="description";
                        label.visually-hidden for="edit-todo-input" { "Edit Todo Input" }
                    } }
                }
        }
    }
}

pub struct List<'a> {
    pub state: &'a State,
    pub oob: bool,
}

impl<'a, 'b> From<&'a State> for List<'b>
where
    'a: 'b,
{
    fn from(state: &'a State) -> Self {
        List { state, oob: true }
    }
}

impl Render for List<'_> {
    fn render(&self) -> Markup {
        if self.state.todos.is_empty() {
            html! { (TodoPlaceholder::FullPayload) }
        } else {
            let completed = self
                .state
                .todos
                .iter()
                .filter(|todo| todo.completed)
                .count();
            let filtered_todos: Vec<&Todo> = self
                .state
                .todos
                .iter()
                .filter(|todo| match self.state.filter {
                    Filter::All => true,
                    Filter::Active => !todo.completed,
                    Filter::Completed => todo.completed,
                })
                .collect();

            html! { main.main #todo-list hx-swap-oob=[self.oob.then(|| "true")] {
                div.toggle-all-container {
                    input.toggle-all #toggle-all type="checkbox" checked=(completed)
                        hx-post="/toggle-todos";
                    label for="toggle-all" { "Mark all as complete" }
                }

                ul.todo-list {
                    @for todo in filtered_todos{ (todo) }

                    (TodoPlaceholder::Extend)
                }

                (Footer { oob: false, ..Footer::from(self.state) })
            } }
        }
    }
}

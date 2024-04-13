use axum::{
    response::IntoResponse,
    routing::{delete, get, post},
    Form, Router,
};
use maud::{html, Markup, PreEscaped, Render, DOCTYPE};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_sessions::{
    cookie::time::Duration, ExpiredDeletion, Expiry, Session, SessionManagerLayer,
};
use tower_sessions_surrealdb_store::SurrealSessionStore;

const STYLESHEET: &str = include_str!("style.css");
const STATE_KEY: &str = "state";

#[tokio::main]
async fn main() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("Surreal initialization failure");
    db.use_ns("testing")
        .await
        .expect("Surreal namespace initialization failure");
    db.use_db("testing")
        .await
        .expect("Surreal database initialization failure");

    let session_store = SurrealSessionStore::new(db.clone(), "sessions".to_string());
    let expired_session_cleanup_interval: u64 = 1;
    tokio::task::spawn(session_store.clone().continuously_delete_expired(
        tokio::time::Duration::from_secs(60 * expired_session_cleanup_interval),
    ));

    let session_service = ServiceBuilder::new().layer(
        SessionManagerLayer::new(session_store)
            .with_secure(false)
            .with_expiry(Expiry::OnInactivity(Duration::minutes(30))),
    );

    let app = Router::new()
        .route("/", get(handler))
        .merge(Footer::routes())
        .merge(Todo::routes())
        .layer(session_service);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Default, Deserialize, Serialize)]
struct State {
    todos: Vec<Todo>,
    footer: Footer,
}

async fn handler(session: Session) -> impl IntoResponse {
    let state: State = session
        .get(STATE_KEY)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    html! { (DOCTYPE) html lang="en" data-framework="axum-htmx-maud" {
        head {
            meta charset="utf-8";
            meta name="description" content="A demo of TodoMVC using axum, htmx, and maud";
            meta name="viewport" content="width=device-width, initial-scale=1.0";
            meta http-equiv="X-UA-Compatible" content="IE=edge";

            script src="https://unpkg.com/htmx.org@1.9.11" integrity="sha384-0gxUXCCR8yv9FM2b+U3FDbsKthCI66oH5IA9fHppQq9DDMHuMauqq1ZHBpJxQ0J0" crossorigin="anonymous" { }
            script src="https://unpkg.com/htmx.org@1.9.11/dist/ext/alpine-morph.js" { }
            script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js" { }

            style { (PreEscaped(STYLESHEET)) }

            title { "TodoMVC: axum, htmx, and maud" }
        }

        body {
            section.todoapp {
                header.header {
                    h1 { "todos" }
                    input.new-todo
                        placeholder="What needs to be done?"
                        hx-post="/todo"
                        hx-target="input[name='next-todo']"
                        hx-swap="outerHTML"
                        name="todo"
                        hx-include="input[name='next-todo']"
                        autofocus { }
                }

                (Todos(state.todos))

                (state.footer)
            }

            footer.info {
                p { "Double-click to edit a todo" }
                p { "Created by " a href="https://garrettdavis.dev" { "Garrett Davis" } }
                p { "Based on " a href="http://todomvc.com" { "TodoMVC" } }
            }
        }
    } }
}

#[derive(Debug, Deserialize, Serialize)]
enum TodoPlaceholder {
    Extend,
    FullPayload,
}

impl Render for TodoPlaceholder {
    fn render(&self) -> Markup {
        html! { input type="hidden" name="next-todo" value=(match self {
            TodoPlaceholder::Extend => "Extend",
            TodoPlaceholder::FullPayload => "FullPayload",
        }) { } }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Todo {
    completed: bool,
    description: String,
    id: u64,
}

struct Todos(Vec<Todo>);

impl Render for Todos {
    fn render(&self) -> Markup {
        html! { @if self.0.is_empty() {
            (TodoPlaceholder::FullPayload)
        } @else {
            main.main {
                div.toggle-all-container {
                    input.toggle-all #toggle-all type="checkbox" { }
                    label for="toggle-all" { "Mark all as complete" }
                }

                ul.todo-list {
                    @for item in &self.0 { (item) }
                    (TodoPlaceholder::Extend)
                }
            }
        } }
    }
}

impl Todo {
    fn routes() -> Router {
        #[derive(Deserialize)]
        struct NewTodo {
            todo: String,
            #[serde(rename = "next-todo")]
            todo_placeholder: TodoPlaceholder,
        }
        async fn add_todo(session: Session, Form(new_todo): Form<NewTodo>) -> impl IntoResponse {
            let todo = Todo {
                completed: false,
                description: new_todo.todo,
                id: rand::random(),
            };
            let mut state: State = session
                .get(STATE_KEY)
                .await
                .unwrap_or(None)
                .unwrap_or_default();
            state.todos.push(todo.clone());
            session.insert(STATE_KEY, state).await.unwrap();

            match new_todo.todo_placeholder {
                TodoPlaceholder::FullPayload => html! { (Todos(vec![todo])) },
                TodoPlaceholder::Extend => html! { (todo) (TodoPlaceholder::Extend) },
            }
        }

        #[derive(Deserialize)]
        struct TodoId {
            id: u64,
        }
        async fn delete_todo(session: Session, Form(todo_id): Form<TodoId>) -> impl IntoResponse {
            let mut state: State = session
                .get(STATE_KEY)
                .await
                .unwrap_or(None)
                .unwrap_or_default();
            state.todos.retain(|todo| todo.id != todo_id.id);
            session.insert(STATE_KEY, state).await.unwrap();
        }

        Router::new()
            .route("/todo", post(add_todo))
            .route("/todo", delete(delete_todo))
    }

    fn data(&self) -> String {
        format!(
            r#"{{"editing":false,"description":"{}"}}"#,
            self.description
        )
    }
}

impl Render for Todo {
    fn render(&self) -> Markup {
        html! {
            li.completed[self.completed] #(self.id) x-data=(self.data())
                x-bind:class=r#"editing && "editing""# x-on:dblclick="editing = !editing" {
                    div.view x-show="!editing" {
                        input.toggle type="checkbox" { }
                        label x-text="description" { }
                        button.destroy hx-delete="/todo" hx-swap="outerHTML" hx-target="closest li" hx-include="next input.destroy-data" { }
                        input.destroy-data type="hidden" name="id" value=(self.id) { }
                    }

                    div.input-container x-show="editing" {
                        input.edit #edit-todo-input x-model="description" { }
                        label.visually-hidden for="edit-todo-input" { "Edit Todo Input" }
                    }
                }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
enum Filter {
    #[default]
    All,
    Active,
    Completed,
}

impl ToString for Filter {
    fn to_string(&self) -> String {
        ToString::to_string(match self {
            Filter::All => "All",
            Filter::Active => "Active",
            Filter::Completed => "Completed",
        })
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Footer {
    current_filter: Filter,
    num_active: u16,
    num_completed: u16,
}

impl Footer {
    fn routes() -> Router {
        #[derive(Debug, Deserialize)]
        struct SelectForm {
            filter: Filter,
        }
        async fn select_filter(session: Session, Form(q): Form<SelectForm>) -> impl IntoResponse {
            let mut state: State = session
                .get(STATE_KEY)
                .await
                .unwrap_or(None)
                .unwrap_or_default();
            state.footer.current_filter = q.filter.clone();
            session.insert(STATE_KEY, state).await.unwrap();

            html! { (Footer { current_filter: q.filter, ..Default::default() }) }
        }

        Router::new().route("/select", post(select_filter))
    }
}

impl Render for Footer {
    fn render(&self) -> Markup {
        html! { footer.footer {
            span.todo-count {
                strong { (self.num_active) }
                " item" @if self.num_active != 1 { "s" } " left"
            }

            ul.filters {
                @for filter in [Filter::All, Filter::Active, Filter::Completed] {
                    li {
                        a.selected[self.current_filter == filter]
                            hx-post=("/select")
                            hx-target="footer"
                            hx-include="find input"
                            hx-swap="outerHTML" {
                                input type="hidden" name="filter" value=(filter.to_string()) { }
                                (filter.to_string())
                            }
                    }
                }
            }

            @if self.num_completed > 0 {
                button.clear-completed { "Clear completed" }
            }
        } }
    }
}

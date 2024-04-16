use std::sync::atomic::{AtomicU64, Ordering};

use axum::{
    extract::Path,
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Form, Router,
};
use maud::{html, PreEscaped, DOCTYPE};
use serde::Deserialize;
use todos::TodoPlaceholder;
use tower::ServiceBuilder;
use tower_sessions::{
    cookie::time::Duration, ExpiredDeletion, Expiry, Session, SessionManagerLayer,
};
use tower_sessions_surrealdb_store::SurrealSessionStore;

use crate::{
    filter::Filter,
    footer::Footer,
    state::State,
    todos::{List, Todo},
};

mod filter;
mod footer;
mod state;
mod todos;

const STYLESHEET: &str = include_str!("style.css");
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);
fn get_id() -> u64 {
    ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

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
        .route("/", get(index))
        .route("/clear-completed", post(clear_completed))
        .route("/select", post(select_filter))
        .route("/todo", post(add_todo))
        .route("/todo/:id", delete(delete_todo))
        .route("/todo/:id", patch(patch_todo))
        .route("/toggle-todos", post(toggle_todos))
        .layer(session_service);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn index(session: Session) -> impl IntoResponse {
    let state = State::read(session).await;

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
                        hx-post="/todo" hx-target="input[name='next-todo']" hx-include="input[name='next-todo']" hx-swap="outerHTML"
                        x-data "x-on:htmx:after-request"="$event.target.value = ''"
                        placeholder="What needs to be done?" name="todo" autofocus;
                }

                (List::from(&state))
            }

            footer.info {
                p { "Double-click to edit a todo" }
                p { "Created by " a href="https://garrettdavis.dev" { "Garrett Davis" } }
                p { "Based on " a href="http://todomvc.com" { "TodoMVC" } }
            }
        }
    } }
}

async fn clear_completed(session: Session) -> impl IntoResponse {
    let mut state = State::read(session.clone()).await;
    state.todos.retain(|todo| !todo.completed);
    state.write(session).await;

    html! { (List::from(&state)) }
}

#[derive(Debug, Deserialize)]
struct SelectForm {
    filter: Filter,
}
async fn select_filter(session: Session, Form(q): Form<SelectForm>) -> impl IntoResponse {
    let mut state = State::read(session.clone()).await;
    state.filter = q.filter;
    state.write(session).await;

    html! { (List::from(&state)) }
}

#[derive(Deserialize)]
struct NewTodo {
    todo: String,
    #[serde(rename = "next-todo")]
    placeholder: TodoPlaceholder,
}
async fn add_todo(session: Session, Form(new_todo): Form<NewTodo>) -> impl IntoResponse {
    let mut state = State::read(session.clone()).await;
    state.todos.push(Todo {
        completed: false,
        description: new_todo.todo,
        id: get_id(),
    });
    state.write(session).await;

    html! { @match new_todo.placeholder {
        TodoPlaceholder::FullPayload => (List { oob: false, ..List::from(&state) }),
        TodoPlaceholder::Extend => (state.todos.last().unwrap()) (Footer::from(&state)) (TodoPlaceholder::Extend),
    } }
}

#[derive(Deserialize)]
struct Id {
    id: u64,
}
async fn delete_todo(session: Session, Path(path): Path<Id>) -> impl IntoResponse {
    let mut state = State::read(session.clone()).await;
    state.todos.retain(|todo| todo.id != path.id);
    let footer = Footer::from(&state);
    state.write(session).await;
    html! { (footer) }
}

#[derive(Debug, Deserialize)]
struct PatchTodo {
    completed: Option<bool>,
    desc: Option<String>,
}
async fn patch_todo(
    session: Session,
    Path(path): Path<Id>,
    Form(body): Form<PatchTodo>,
) -> impl IntoResponse {
    let mut state = State::read(session.clone()).await;

    if let Some(todo) = state.todos.iter_mut().find(|todo| todo.id == path.id) {
        if let Some(completed) = body.completed {
            todo.completed = !completed; // toggle the value
        }
        if let Some(description) = body.desc {
            todo.description = description;
        }

        let result = html! { (todo) (Footer::from(&state)) };
        state.write(session).await;

        result
    } else {
        html! {}
    }
}

async fn toggle_todos(session: Session) -> impl IntoResponse {
    let mut state = State::read(session.clone()).await;
    let all_completed = state.todos.iter().all(|todo| todo.completed);
    state
        .todos
        .iter_mut()
        .for_each(|todo| todo.completed = !all_completed);
    state.write(session).await;
    html! { (List::from(&state)) }
}

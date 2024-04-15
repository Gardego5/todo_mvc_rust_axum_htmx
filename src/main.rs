use axum::{
    extract::Path,
    response::IntoResponse,
    routing::{delete, get, patch, post},
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
    filter: Filter,
}

impl State {
    const KEY: &'static str = "state";

    async fn read(session: Session) -> Self {
        session
            .get(Self::KEY)
            .await
            .unwrap_or(None)
            .unwrap_or_default()
    }

    async fn write(&self, session: Session) {
        session.insert(Self::KEY, self).await.unwrap();
    }
}

async fn handler(session: Session) -> impl IntoResponse {
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
                        x-data "@htmx:after-request.camel"="$event.detail.successful && ($event.target.value = '')"
                        placeholder="What needs to be done?" name="todo" autofocus;
                }

                (List::from(&state))

                (Footer::from(&state))
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
        html! { @match self {
            TodoPlaceholder::Extend => input type="hidden" name="next-todo" value="Extend";,
            TodoPlaceholder::FullPayload => input #todo-list hx-swap-oob="true"
                type="hidden" name="next-todo" value="FullPayload";
        } }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Todo {
    completed: bool,
    description: String,
    id: u64,
}

struct List<'a> {
    todos: Vec<&'a Todo>,
    all_completed: bool,
    oob: bool,
}

impl<'a, 'b> From<&'a State> for List<'b>
where
    'a: 'b,
{
    fn from(state: &'a State) -> Self {
        List {
            oob: true,
            all_completed: state.todos.iter().all(|todo| todo.completed),
            todos: match state.filter {
                Filter::All => state.todos.iter().collect(),
                Filter::Active => state.todos.iter().filter(|todo| !todo.completed).collect(),
                Filter::Completed => state.todos.iter().filter(|todo| todo.completed).collect(),
            },
        }
    }
}

impl Render for List<'_> {
    fn render(&self) -> Markup {
        html! { @if self.todos.is_empty() {
            (TodoPlaceholder::FullPayload)
        } @else {
            main.main #todo-list hx-swap-oob=[self.oob.then(|| "true")] {
                div.toggle-all-container {
                    input.toggle-all #toggle-all type="checkbox" checked=(self.all_completed)
                        hx-post="/toggle-todos";
                    label for="toggle-all" { "Mark all as complete" }
                }

                ul.todo-list {
                    @for todo in &self.todos { (todo) }
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
            placeholder: TodoPlaceholder,
        }
        async fn add_todo(session: Session, Form(new_todo): Form<NewTodo>) -> impl IntoResponse {
            let mut state = State::read(session.clone()).await;
            state.todos.push(Todo {
                completed: false,
                description: new_todo.todo,
                id: rand::random(),
            });
            state.write(session).await;

            html! { @match new_todo.placeholder {
                TodoPlaceholder::FullPayload => (List { oob: false, ..List::from(&state) }) (Footer::from(&state)),
                TodoPlaceholder::Extend => (state.todos.last().unwrap()) (TodoPlaceholder::Extend),
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
            html! { (List::from(&state)) (Footer::from(&state)) }
        }

        Router::new()
            .route("/todo", post(add_todo))
            .route("/todo/:id", delete(delete_todo))
            .route("/todo/:id", patch(patch_todo))
            .route("/toggle-todos", post(toggle_todos))
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
            li.completed[self.completed] #{"todo-" (self.id)} x-data=(self.data())
                x-bind:class=r#"editing && "editing""#
                x-on:dblclick="editing = !editing; $nextTick(() => $refs['edit-todo-input'].focus())"
                hx-swap="outerHTML" hx-target={"#todo-" (self.id)} {
                    div.view x-show="!editing" {
                        input.toggle type="checkbox" checked[self.completed]
                            hx-patch={"/todo/" (self.id)} hx-include="next input[name='completed']";
                        label x-text="description" { }
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
    num_active: usize,
    num_completed: usize,
    oob: bool,
}

impl Footer {
    fn routes() -> Router {
        #[derive(Debug, Deserialize)]
        struct SelectForm {
            filter: Filter,
        }
        async fn select_filter(session: Session, Form(q): Form<SelectForm>) -> impl IntoResponse {
            let mut state = State::read(session.clone()).await;
            state.filter = q.filter;
            state.write(session).await;

            html! { (List::from(&state)) (Footer { oob: false, ..From::from(&state) }) }
        }

        async fn clear_completed(session: Session) -> impl IntoResponse {
            let mut state = State::read(session.clone()).await;
            state.todos.retain(|todo| !todo.completed);
            state.write(session).await;

            html! { (List::from(&state)) (Footer { oob: false, ..From::from(&state) }) }
        }

        Router::new()
            .route("/select", post(select_filter))
            .route("/clear-completed", post(clear_completed))
    }
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

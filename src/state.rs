use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{filter::Filter, todos::Todo};

#[derive(Default, Deserialize, Serialize)]
pub struct State {
    pub todos: Vec<Todo>,
    pub filter: Filter,
}

impl State {
    const KEY: &'static str = "state";

    pub async fn read(session: Session) -> Self {
        session
            .get(Self::KEY)
            .await
            .unwrap_or(None)
            .unwrap_or_default()
    }

    pub async fn write(&self, session: Session) {
        session.insert(Self::KEY, self).await.unwrap();
    }
}

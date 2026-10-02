use std::{sync::Arc, time::Duration};

use axum::{
    extract::State,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures_util::Stream;
use serde_json::{Value, json};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    BuildServerState,
    job::{JobTaskKind, JobTaskStatus},
};

#[derive(Clone)]
pub enum BuildServerEvent {
    JobStart(u64),
    JobEnd,
    TaskRegister(usize, JobTaskKind, String),
    TaskStatus(usize, JobTaskStatus),
}

impl BuildServerEvent {
    fn event(&self) -> &str {
        match self {
            Self::JobStart(_) => "job_start",
            Self::JobEnd => "job_end",
            Self::TaskRegister(..) => "task_register",
            Self::TaskStatus(..) => "task_status",
        }
    }

    fn payload(&self) -> Value {
        match self {
            Self::JobStart(id) => json!({ "id": id }),
            Self::JobEnd => Value::Null,
            Self::TaskRegister(id, kind, name) => json!({ "id": id, "type": kind.to_string(), "name": name}),
            Self::TaskStatus(id, status) => json!({ "id": id, "status": status.to_string()}),
        }
    }
}

pub async fn get_events(State(state): State<Arc<BuildServerState>>) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let rx = state.event_channel.subscribe();
    let stream = BroadcastStream::new(rx).map(|msg| match msg {
        Ok(event) => Event::default().event(event.event()).json_data(event.payload()),
        Err(_) => Ok(Event::default().event("resync").data("{}")),
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)).text("keep-alive-text"))
}

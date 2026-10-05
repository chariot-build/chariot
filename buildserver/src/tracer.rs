use std::{io::stdout, sync::Arc};

use chariot_core::{
    config::package::PackagePlatform,
    graph::{TaskId, TaskKind},
    tracer::{CapturingLogger, Logger, PackageStep, PrepareStep, SourceStep, TaskStatus, Tracer},
};

use crate::{
    BuildServerEvent, BuildServerState,
    job::{Job, JobTask, JobTaskKind, JobTaskStatus},
};

pub struct BuildServerTracer {
    state: Arc<BuildServerState>,
    job: Arc<Job>,
}

impl BuildServerTracer {
    pub fn new(state: Arc<BuildServerState>, job: Arc<Job>) -> Self {
        Self { state, job }
    }
}

impl Tracer for BuildServerTracer {
    fn register_task(&self, id: TaskId, kind: &TaskKind) {
        let name = match kind {
            TaskKind::Source { source, .. } => source.name.clone(),
            TaskKind::Package { package, .. } => package.name.clone(),
        };

        let input_hash = match kind {
            TaskKind::Package { package, .. } => package.get_content_hash(),
            TaskKind::Source { source, .. } => source.get_prepare_hash(source.get_prepare_base_hash(source.get_patch_hash(source.get_base_hash()))),
        };

        let kind = match kind {
            TaskKind::Source { .. } => JobTaskKind::Source,
            TaskKind::Package { package, .. } => match package.platform {
                PackagePlatform::Target => JobTaskKind::Package,
                PackagePlatform::Host => JobTaskKind::Tool,
            },
        };

        let database_id = self
            .state
            .db
            .create_task(self.job.id, &name, kind, JobTaskStatus::Pending, input_hash)
            .expect("Failed to create database entry for task");

        let task = JobTask {
            id: database_id,
            name: name,
            kind: kind,
            status: JobTaskStatus::Pending,
            input_hash,
        };

        let _ = self
            .state
            .event_channel
            .send(BuildServerEvent::TaskRegister(id.into(), task.kind, task.name.clone()));

        self.job.tasks.write().unwrap().insert(id.into(), task);
    }

    fn task_status(&self, id: TaskId, status: TaskStatus) {
        let task_status = match status {
            TaskStatus::Started => JobTaskStatus::InProgress,
            TaskStatus::Finished => JobTaskStatus::Succeeded,
            TaskStatus::CacheHit => JobTaskStatus::CacheHit,
            TaskStatus::Skipped => JobTaskStatus::Skipped,
            TaskStatus::Failed { .. } => JobTaskStatus::Failed,
        };

        let _ = self.state.event_channel.send(BuildServerEvent::TaskStatus(id.into(), task_status));

        if let Some(task) = self.job.tasks.write().unwrap().get_mut(&id.into()) {
            task.status = task_status;
            self.state
                .db
                .update_task_status(task.id, task_status)
                .expect("Failed to update task state");
        }
    }

    fn package_step(&self, id: TaskId, step: PackageStep) -> Box<dyn Logger> {
        Box::new(CapturingLogger::new(stdout()))
    }

    fn source_step(&self, id: TaskId, step: SourceStep) -> Box<dyn Logger> {
        Box::new(CapturingLogger::new(stdout()))
    }

    fn prepare_step(&self, id: TaskId, step: PrepareStep) -> Box<dyn Logger> {
        Box::new(CapturingLogger::new(stdout()))
    }
}

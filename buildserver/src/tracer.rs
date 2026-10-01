use std::io::stdout;

use chariot_core::{
    graph::{TaskId, TaskKind},
    tracer::{CapturingLogger, Logger, PackageStep, PrepareStep, SourceStep, TaskStatus, Tracer},
};

pub struct BuildServerTracer {}

impl BuildServerTracer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tracer for BuildServerTracer {
    fn register_task(&self, id: TaskId, kind: &TaskKind, referenced_by: &[(TaskId, String)]) {}

    fn task_status(&self, id: TaskId, status: TaskStatus) {}

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

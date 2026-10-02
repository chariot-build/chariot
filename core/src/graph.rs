use std::{
    collections::{HashMap, HashSet},
    iter,
    sync::Arc,
};

use crate::config::{package::Package, source::Source};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(usize);

impl From<TaskId> for usize {
    fn from(value: TaskId) -> Self {
        value.0
    }
}

#[derive(Clone, Debug)]
pub enum TaskKind {
    Package { package: Arc<Package>, runtime_dep_edges: Vec<TaskId> },
    Source { source: Arc<Source> },
}

struct TaskNode {
    kind: TaskKind,
    dependencies: Vec<TaskId>,
}

pub struct BuildGraph {
    nodes: Vec<TaskNode>,
    package_ids: HashMap<usize, TaskId>,
    source_ids: HashMap<usize, TaskId>,
}

impl BuildGraph {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn ids(&self) -> impl Iterator<Item = TaskId> + '_ {
        (0..self.nodes.len()).map(TaskId)
    }

    pub fn dependencies(&self, id: TaskId) -> &[TaskId] {
        &self.nodes[id.0].dependencies
    }

    pub fn kind(&self, id: TaskId) -> TaskKind {
        self.nodes[id.0].kind.clone()
    }

    pub fn task_for_package(&self, pkg: &Arc<Package>) -> TaskId {
        self.package_ids[&(Arc::as_ptr(pkg) as usize)]
    }

    pub fn task_for_source(&self, source: &Arc<Source>) -> TaskId {
        self.source_ids[&(Arc::as_ptr(source) as usize)]
    }

    pub fn runtime_dependency_closure(&self, pkg_task: TaskId) -> Vec<TaskId> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        self.closure_visit(pkg_task, &mut out, &mut seen);
        out
    }

    fn closure_visit(&self, id: TaskId, out: &mut Vec<TaskId>, seen: &mut HashSet<TaskId>) {
        if !seen.insert(id) {
            return;
        }
        out.push(id);

        let runtime_dep_edges = match &self.nodes[id.0].kind {
            TaskKind::Package { runtime_dep_edges, .. } => runtime_dep_edges,
            _ => unreachable!(),
        };

        for &dep in runtime_dep_edges {
            self.closure_visit(dep, out, seen);
        }
    }
}

pub struct BuildGraphBuilder {
    graph: BuildGraph,
}

impl BuildGraphBuilder {
    pub fn new() -> Self {
        Self {
            graph: BuildGraph {
                nodes: Vec::new(),
                package_ids: HashMap::new(),
                source_ids: HashMap::new(),
            },
        }
    }

    pub fn add_root_package(&mut self, pkg: &Arc<Package>) -> TaskId {
        self.visit_package(pkg)
    }

    pub fn add_root_source(&mut self, source: &Arc<Source>) -> TaskId {
        self.visit_source(source)
    }

    pub fn finish(self) -> BuildGraph {
        self.graph
    }

    fn alloc(&mut self, kind: TaskKind) -> TaskId {
        let id = TaskId(self.graph.nodes.len());
        self.graph.nodes.push(TaskNode {
            kind,
            dependencies: Vec::new(),
        });
        id
    }

    fn visit_package(&mut self, pkg: &Arc<Package>) -> TaskId {
        let ptr = Arc::as_ptr(pkg) as usize;
        if let Some(&id) = self.graph.package_ids.get(&ptr) {
            return id;
        }

        let id = self.alloc(TaskKind::Package {
            package: pkg.clone(),
            runtime_dep_edges: Vec::new(),
        });
        self.graph.package_ids.insert(ptr, id);

        for dependency in &pkg.dependencies.sources {
            let source_id = self.visit_source(dependency);
            self.graph.nodes[id.0].dependencies.push(source_id);
        }

        for dependency in iter::chain(&pkg.dependencies.packages, &pkg.dependencies.tools) {
            let dependency_id = self.visit_package(dependency);
            let closure = self.graph.runtime_dependency_closure(dependency_id);
            self.graph.nodes[id.0].dependencies.extend(closure);
        }

        for runtime_dependency in &pkg.runtime_dependencies {
            let dependency_id = self.visit_package(runtime_dependency);
            let TaskKind::Package { runtime_dep_edges, .. } = &mut self.graph.nodes[id.0].kind else {
                unreachable!();
            };
            runtime_dep_edges.push(dependency_id);
        }

        id
    }

    fn visit_source(&mut self, source: &Arc<Source>) -> TaskId {
        let ptr = Arc::as_ptr(source) as usize;
        if let Some(&id) = self.graph.source_ids.get(&ptr) {
            return id;
        }

        let id = self.alloc(TaskKind::Source { source: source.clone() });
        self.graph.source_ids.insert(ptr, id);

        if let Some(prepare) = &source.prepare {
            for dependency in &prepare.dependencies.sources {
                let source_id = self.visit_source(dependency);
                self.graph.nodes[id.0].dependencies.push(source_id);
            }

            for dependency in iter::chain(&prepare.dependencies.packages, &prepare.dependencies.tools) {
                let dependency_id = self.visit_package(dependency);
                let closure = self.graph.runtime_dependency_closure(dependency_id);
                self.graph.nodes[id.0].dependencies.extend(closure);
            }
        }

        id
    }
}

impl Default for BuildGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

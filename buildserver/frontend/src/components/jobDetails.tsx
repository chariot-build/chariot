import { useEffect, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import {
    CircleCheck,
    CircleMinus,
    CircleX,
    Clock,
    DatabaseZap,
    LoaderCircle,
} from "lucide-preact";
import { fetchJson } from "../utils/fetch";
import { useBuildEvents } from "../utils/events";
import { cn } from "cn";
import { FileList } from "./fileList";
import Skeleton from "./skeleton";
import { TaskLogs } from "./taskLogs";
import { navigate, parseRoute, routePath, usePathname } from "../utils/router";

type Task = {
    id: number;
    type: "source" | "package" | "tool";
    name: string;
    input_hash: string;
    status:
        | "pending"
        | "in_progress"
        | "failed"
        | "succeeded"
        | "skipped"
        | "cache_hit";
};

function TaskStatusIcon({ status }: { status: Task["status"] }) {
    const base = "size-5 shrink-0";

    switch (status) {
        case "in_progress":
            return (
                <LoaderCircle className={`${base} animate-spin text-accent`} />
            );
        case "succeeded":
            return <CircleCheck className={`${base} text-ok`} />;
        case "failed":
            return <CircleX className={`${base} text-err`} />;
        case "pending":
            return <Clock className={`${base} text-muted`} />;
        case "skipped":
            return <CircleMinus className={`${base} text-muted`} />;
        case "cache_hit":
            return <DatabaseZap className={`${base} text-warn`} />;
    }
}

function JobRow({
    task,
    selectedTask,
    setSelectedTask,
}: {
    task: Task;
    selectedTask: number | null;
    setSelectedTask: (task: number) => void;
}) {
    return (
        <button
            onClick={() => setSelectedTask(task.id)}
            className={cn(
                "flex w-full cursor-pointer items-center gap-4 rounded-md px-4 py-3 text-left transition-colors hover:bg-elevated",
                selectedTask === task.id && "bg-elevated",
            )}
        >
            <span className="w-20 shrink-0 text-sm text-muted">
                {task.type}
            </span>
            <span className="min-w-0 flex-1 truncate text-base text-fg">
                {task.name}
            </span>
            <span className="inline-flex shrink-0 items-center gap-1.5 text-sm text-muted">
                <TaskStatusIcon status={task.status} />
                {task.status}
            </span>
        </button>
    );
}

function TabButton({
    active,
    onClick,
    children,
}: {
    active: boolean;
    onClick: () => void;
    children: ComponentChildren;
}) {
    return (
        <button
            onClick={onClick}
            className={cn(
                "cursor-pointer border-b pb-1 transition-colors",
                active
                    ? "border-fg text-fg"
                    : "border-transparent text-muted hover:text-fg",
            )}
        >
            {children}
        </button>
    );
}

export function JobDetails({
    selectedJob,
    refreshToken,
}: {
    selectedJob: string;
    refreshToken: number;
}) {
    let [tasks, setTasks] = useState<Task[]>([]);
    let [loading, setLoading] = useState(true);
    let [taskHash, setTaskHash] = useState<string | null>(null);
    let [tab, setTab] = useState<"logs" | "files">("logs");

    const route = parseRoute(usePathname());
    const selectedTask = route.task === null ? null : Number(route.task);

    const setSelectedTask = (task: number | null) =>
        navigate(
            routePath({
                project: route.project,
                job: route.job,
                task: task === null ? null : String(task),
            }),
        );

    const selectedTaskData = tasks.find((task) => task.id === selectedTask);

    useEffect(() => {
        setTab("logs");
    }, [selectedTask]);

    useEffect(() => {
        setTasks([]);
        setLoading(true);
    }, [selectedJob]);

    useEffect(() => {
        setTaskHash(null);
        if (selectedTaskData === undefined) return;

        const fetchData = async () => {
            try {
                const result = await fetchJson(
                    `/ledger/lookup/install/${selectedTaskData.input_hash}`,
                );
                setTaskHash(result.output_hash);
            } catch (err) {
                console.error(err);
            }
        };

        fetchData();
    }, [selectedTaskData?.input_hash]);

    useEffect(() => {
        const fetchData = async () => {
            try {
                const result = await fetchJson(`/job/${selectedJob}/details`);
                setTasks(result.tasks);
            } catch (err) {
                console.error(err);
            } finally {
                setLoading(false);
            }
        };

        fetchData();
    }, [selectedJob, refreshToken]);

    useBuildEvents((event) => {
        const jobId = Number(selectedJob);

        if (event.type === "task_register" && event.jobId === jobId) {
            setTasks((current) => {
                if (current.some((task) => task.id === event.taskId)) {
                    return current;
                }

                return [
                    ...current,
                    {
                        id: event.taskId,
                        type: event.taskType as Task["type"],
                        name: event.name,
                        input_hash: event.input_hash,
                        status: "pending",
                    },
                ];
            });
        } else if (event.type === "task_status" && event.jobId === jobId) {
            setTasks((current) =>
                current.map((task) =>
                    task.id === event.taskId
                        ? { ...task, status: event.status as Task["status"] }
                        : task,
                ),
            );
        }
    });

    return (
        <div className="flex w-full flex-col gap-8">
            <div className="flex flex-col gap-8 lg:flex-row lg:items-start">
                <div className="flex min-w-0 flex-1 flex-col gap-1 lg:max-w-md">
                    {loading
                        ? [1, 2, 3].map((item) => (
                              <div key={item} className="px-4 py-3">
                                  <Skeleton
                                      variant="text"
                                      className="h-5 w-full"
                                  />
                              </div>
                          ))
                        : tasks.map((task) => (
                              <JobRow
                                  key={task.id}
                                  selectedTask={selectedTask}
                                  setSelectedTask={setSelectedTask}
                                  task={task}
                              />
                          ))}
                </div>

                <div className="h-0.5 w-full shrink-0 bg-line lg:h-auto lg:w-0.5 lg:self-stretch" />

                <div className="flex min-w-0 flex-1 flex-col gap-4">
                    {selectedTaskData === undefined ? (
                        <div className="py-10 text-sm text-muted">
                            Select a task to view its logs.
                        </div>
                    ) : (
                        <>
                            <div className="flex items-baseline justify-between gap-4">
                                <div className="min-w-0">
                                    <div className="truncate text-base text-fg">
                                        {selectedTaskData.name}
                                    </div>
                                    <div className="text-sm text-muted">
                                        {selectedTaskData.type}
                                    </div>
                                </div>
                                <span className="inline-flex shrink-0 items-center gap-1.5 text-sm text-muted">
                                    <TaskStatusIcon
                                        status={selectedTaskData.status}
                                    />
                                    {selectedTaskData.status}
                                </span>
                            </div>

                            <div className="flex items-center gap-4 text-sm">
                                <TabButton
                                    active={tab === "logs"}
                                    onClick={() => setTab("logs")}
                                >
                                    Logs
                                </TabButton>
                                <TabButton
                                    active={tab === "files"}
                                    onClick={() => setTab("files")}
                                >
                                    Files
                                </TabButton>
                            </div>

                            {tab === "logs" ? (
                                <TaskLogs
                                    id={selectedTaskData.id}
                                    name={selectedTaskData.name}
                                    type={selectedTaskData.type}
                                    status={selectedTaskData.status}
                                />
                            ) : taskHash !== null ? (
                                <FileList hash={taskHash} />
                            ) : (
                                <div className="py-10 text-sm text-muted">
                                    No files available.
                                </div>
                            )}
                        </>
                    )}
                </div>
            </div>
        </div>
    );
}

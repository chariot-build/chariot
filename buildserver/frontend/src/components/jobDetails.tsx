import { useEffect, useState } from "preact/hooks";
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
    const base = "size-4 shrink-0";

    switch (status) {
        case "in_progress":
            return (
                <LoaderCircle
                    className={`${base} animate-spin text-[#5b8cff]`}
                />
            );
        case "succeeded":
            return <CircleCheck className={`${base} text-[#3fb950]`} />;
        case "failed":
            return <CircleX className={`${base} text-[#f85149]`} />;
        case "pending":
            return <Clock className={`${base} text-[#9a9a9a]`} />;
        case "skipped":
            return <CircleMinus className={`${base} text-[#6e6e6e]`} />;
        case "cache_hit":
            return <DatabaseZap className={`${base} text-[#d29922]`} />;
    }
}

function JobTableEntry({
    task,
    selectedTask,
    setSelectedTask,
}: {
    task: Task;
    selectedTask: number | null;
    setSelectedTask: (task: number) => void;
}) {
    return (
        <tr
            onClick={() => setSelectedTask(task.id)}
            className={cn(
                "border-b border-[#2a2a2a] last:border-b-0 hover:bg-[#242424]",
                selectedTask == task.id &&
                    "bg-[#242424] border-l-[#5b8cff] text-white font-semibold",
            )}
        >
            <td className="px-4 py-2.5 text-sm text-[#c9c9c9] first:w-35 first:text-xs first:font-medium first:uppercase first:tracking-[0.03em] first:text-[#9a9a9a] last:text-right last:font-semibold">
                {task.type}
            </td>
            <td className="px-4 py-2.5 text-sm text-[#c9c9c9] first:w-35 first:text-xs first:font-medium first:uppercase first:tracking-[0.03em] first:text-[#9a9a9a] last:text-right last:font-semibold">
                {task.name}
            </td>
            <td className="px-4 py-2.5 text-sm text-[#c9c9c9] first:w-35 first:text-xs first:font-medium first:uppercase first:tracking-[0.03em] first:text-[#9a9a9a] last:text-right last:font-semibold">
                <span className="inline-flex w-full items-center justify-end gap-1.5">
                    <TaskStatusIcon status={task.status} />
                    {task.status}
                </span>
            </td>
        </tr>
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
    let [taskHash, setTaskHash] = useState<string | null>(null);

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
        setTasks([]);
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
        <div className="w-full flex flex-row items-start gap-10">
            <div className="flex-1 min-w-0 max-w-180">
                <table className="w-full border-collapse bg-[#1c1c1c] border border-[#2e2e2e] rounded-lg overflow-hidden">
                    <tbody>
                        <tr className="border-b border-[#2a2a2a] last:border-b-0">
                            <td className="px-4 py-2.5 first:w-35 text-xs font-medium lowercase tracking-[0.03em] text-[#9a9a9a] last:text-right">
                                Type
                            </td>
                            <td className="px-4 py-2.5 first:w-35 text-xs font-medium lowercase tracking-[0.03em] text-[#9a9a9a] last:text-right">
                                Name
                            </td>
                            <td className="px-4 py-2.5 first:w-35 text-xs font-medium lowercase tracking-[0.03em] text-[#9a9a9a] last:text-right">
                                Status
                            </td>
                        </tr>

                        {tasks.map((task) => (
                            <JobTableEntry
                                key={task.id}
                                selectedTask={selectedTask}
                                setSelectedTask={setSelectedTask}
                                task={task}
                            />
                        ))}
                    </tbody>
                </table>
            </div>
            {taskHash !== null && (
                <div className="flex-1 min-w-0 max-w-180">
                    <FileList hash={taskHash} />
                </div>
            )}
        </div>
    );
}

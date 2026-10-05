import { useEffect, useState } from "preact/hooks";
import { fetchJson } from "../utils/fetch";
import { useBuildEvents } from "../utils/events";
import { cn } from "cn";
import { FileList } from "./fileList";

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
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} animate-spin text-[#5b8cff]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="3"
                    strokeLinecap="round"
                >
                    <circle cx="12" cy="12" r="9" className="opacity-20" />
                    <path d="M21 12a9 9 0 0 0-9-9" />
                </svg>
            );
        case "succeeded":
            return (
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} text-[#3fb950]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                >
                    <circle cx="12" cy="12" r="9" />
                    <path d="m8.5 12.5 2.5 2.5 5-5" />
                </svg>
            );
        case "failed":
            return (
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} text-[#f85149]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                >
                    <circle cx="12" cy="12" r="9" />
                    <path d="m9 9 6 6M15 9l-6 6" />
                </svg>
            );
        case "pending":
            return (
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} text-[#9a9a9a]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                >
                    <circle cx="12" cy="12" r="9" />
                    <path d="M12 7v5l3 2" />
                </svg>
            );
        case "skipped":
            return (
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} text-[#6e6e6e]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                >
                    <circle cx="12" cy="12" r="9" />
                    <path d="M8 12h8" />
                </svg>
            );
        case "cache_hit":
            return (
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} text-[#d29922]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                >
                    <circle cx="12" cy="12" r="9" />
                    <path d="M13 7.5 9.5 12.5H12l-1 4 3.5-5H12l1-4Z" />
                </svg>
            );
    }
}

function JobTableEntry({
    task,
    selectedTask,
    setSelectedTask,
}: {
    task: Task;
    selectedTask: number;
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
    let [selectedTask, setSelectedTask] = useState<number | null>(null);
    let [taskHash, setTaskHash] = useState<string | null>(null);

    useEffect(() => {
        setTasks([]);
        setSelectedTask(null);
    }, [selectedJob]);

    useEffect(() => {
        setTaskHash(null);
        let task = tasks.find((task) => task.id === selectedTask);
        if (task === undefined) return;

        const fetchData = async () => {
            try {
                const result = await fetchJson(
                    `/ledger/lookup/install/${task.input_hash}`,
                );
                setTaskHash(result.output_hash);
            } catch (err) {
                console.error(err);
            }
        };

        fetchData();
    }, [selectedTask]);

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

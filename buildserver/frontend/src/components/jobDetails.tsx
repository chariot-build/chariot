import { useEffect, useState } from "preact/hooks";
import { fetchJson } from "../utils/fetch";
import { useBuildEvents } from "../utils/events";

type Task = {
    id: number;
    type: "source" | "package" | "tool";
    name: string;
    status: "pending" | "in_progress" | "failed" | "succeeded" | "skipped";
};

function JobTableEntry({ task }: { task: Task }) {
    return (
        <tr className="border-b border-[#2a2a2a] last:border-b-0 hover:bg-[#242424]">
            <td className="px-4 py-2.5 text-sm text-[#c9c9c9] first:w-35 first:text-xs first:font-medium first:uppercase first:tracking-[0.03em] first:text-[#9a9a9a] last:text-right last:font-semibold">
                {task.type}
            </td>
            <td className="px-4 py-2.5 text-sm text-[#c9c9c9] first:w-35 first:text-xs first:font-medium first:uppercase first:tracking-[0.03em] first:text-[#9a9a9a] last:text-right last:font-semibold">
                {task.name}
            </td>
            <td className="px-4 py-2.5 text-sm text-[#c9c9c9] first:w-35 first:text-xs first:font-medium first:uppercase first:tracking-[0.03em] first:text-[#9a9a9a] last:text-right last:font-semibold">
                {task.status}
            </td>
        </tr>
    );
}

export function JobDetails({
    selectedProject,
    selectedJob,
    refreshToken,
}: {
    selectedProject: string | null;
    selectedJob: string;
    refreshToken: number;
}) {
    let [tasks, setTasks] = useState<Task[]>([]);

    useEffect(() => {
        setTasks([]);
    }, [selectedJob]);

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
        <table className="w-full max-w-180 border-collapse bg-[#1c1c1c] border border-[#2e2e2e] rounded-lg overflow-hidden">
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
                    <JobTableEntry task={task} />
                ))}
            </tbody>
        </table>
    );
}

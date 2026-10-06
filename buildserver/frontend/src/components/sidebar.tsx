import { useEffect, useRef, useState } from "preact/hooks";
import { ChevronDown } from "lucide-preact";
import type { Project } from "../app";
import Skeleton from "./skeleton";
import { cn } from "cn";
import { fetchJson } from "../utils/fetch";

export function Sidebar({
    projects,
    selectedProject,
    setSelectedProject,
    selectedJob,
    setSelectedJob,
    refreshToken,
}: {
    projects: Project[] | null;
    selectedProject: string | null;
    setSelectedProject: (value: string | null) => void;
    selectedJob: string | null;
    setSelectedJob: (value: string | null) => void;
    refreshToken: number;
}) {
    let [jobs, setJobs] = useState<string[] | null>(null);
    let [open, setOpen] = useState(false);
    const menuRef = useRef<HTMLDivElement>(null);

    let jobs_elements;
    if (jobs !== null) {
        jobs_elements = jobs.toReversed().map((job) => (
            <button
                key={job}
                className={cn(
                    "w-[calc(100%-0.625rem)] text-left ml-2.5 px-2.5 py-1.5 pl-4 border-l-2 border-[#333] rounded-r-1.5 text-[13px] text-[#9a9a9a] hover:bg-[#242424] hover:border-l-[#5b8cff] hover:text-white ",
                    selectedJob == job &&
                        "bg-[#242424] border-l-[#5b8cff] text-white font-semibold",
                )}
                onClick={() => setSelectedJob(job)}
            >
                {job}
            </button>
        ));
    } else {
        jobs_elements = [1, 2, 3].map((job) => (
            <div
                key={job}
                className="w-[calc(100%-0.625rem)] ml-2.5 px-2.5 py-1.5"
            >
                <Skeleton variant="text" className="h-4 w-full" />
            </div>
        ));
    }

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (
                menuRef.current !== null &&
                !menuRef.current.contains(event.target as Node)
            ) {
                setOpen(false);
            }
        };

        document.addEventListener("mousedown", handleClickOutside);

        return () =>
            document.removeEventListener("mousedown", handleClickOutside);
    }, []);

    useEffect(() => {
        setJobs(null);
        setOpen(false);
    }, [selectedProject]);

    useEffect(() => {
        if (selectedProject === null) {
            return;
        }

        const fetchData = async () => {
            try {
                const result = await fetchJson(
                    `/project/${selectedProject}/jobs`,
                );
                setJobs(result.jobs);
            } catch (err) {
                console.error(err);
            }
        };

        fetchData();
    }, [selectedProject, refreshToken]);

    return (
        <div className="flex flex-col w-65 shrink-0 min-h-screen bg-[#1c1c1c] border-r border-solid border-[#1a1a1a] py-4 px-3 gap-0.5">
            <div className="relative mt-1 mb-4" ref={menuRef}>
                <button
                    type="button"
                    onClick={() => setOpen((value) => !value)}
                    disabled={projects === null}
                    className="flex w-full items-center justify-between gap-2 px-3 py-2 pb-3.5 text-[15px] font-semibold tracking-[0.01em] text-[#f2f2f2] border-b border-solid border-[#2e2e2e] cursor-pointer transition-[background-color,color] duration-150 ease-in-out hover:bg-[#2a2a2a] hover:text-white focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-[#5b8cff] focus-visible:-outline-offset-2 disabled:cursor-default disabled:hover:bg-transparent"
                >
                    <span className="truncate">
                        {selectedProject ?? "Chariot Build Server"}
                    </span>
                    <ChevronDown
                        className={cn(
                            "size-4 shrink-0 transition-transform duration-150",
                            open && "rotate-180",
                        )}
                    />
                </button>
                {open && projects !== null && (
                    <div className="absolute left-0 right-0 top-full z-10 mt-1 rounded-md border border-[#2e2e2e] bg-[#242424] py-1 shadow-lg">
                        {projects.map((project) => (
                            <button
                                key={project.name}
                                type="button"
                                onClick={() => {
                                    setSelectedProject(project.name);
                                    setOpen(false);
                                }}
                                className={cn(
                                    "block w-full text-left px-3 py-2 text-sm text-[#c9c9c9] cursor-pointer transition-[background-color,color] duration-150 ease-in-out hover:bg-[#2a2a2a] hover:text-white",
                                    selectedProject === project.name &&
                                        "bg-[#2a2a2a] text-white font-semibold",
                                )}
                            >
                                {project.name}
                            </button>
                        ))}
                    </div>
                )}
            </div>
            <div className="flex flex-col gap-0.5">
                {selectedProject !== null ? jobs_elements : null}
            </div>
        </div>
    );
}

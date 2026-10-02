import { useEffect, useState } from "preact/hooks";
import type { Project } from "../app";
import Skeleton from "./skeleton";
import { cn } from "cn";
import { fetchJson } from "../utils/fetch";

export function SidebarEntry({
    project,
    jobs,
    selectedProject,
    setSelectedProject,
    selectedJob,
    setSelectedJob,
}: {
    project: Project;
    jobs: string[] | null;
    selectedProject: string | null;
    setSelectedProject: (value: string | null) => void;
    selectedJob: string | null;
    setSelectedJob: (value: string | null) => void;
}) {
    let jobs_elements;
    console.log(jobs);
    if (jobs !== null) {
        jobs_elements = jobs.map((job) => (
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
        jobs_elements = [1, 2, 3].map((project) => (
            <div
                key={project}
                className="w-[calc(100%-0.625rem)] ml-2.5 px-2.5 py-1.5"
            >
                <Skeleton variant="text" className="h-4 w-full" />
            </div>
        ));
    }

    let active = selectedProject == project.name;

    return (
        <div className="flex flex-col items-start w-full">
            <button
                className={cn(
                    "block w-full text-left px-3 py-2 rounded-md text-[#c9c9c9] text-sm cursor-pointer transition-[background-color,color] duration-150 ease-in-out hover:bg-[#2a2a2a] hover:text-white focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-[#5b8cff] focus-visible:-outline-offset-2",
                    active && "bg-[#2a2a2a] text-white font-semibold",
                )}
                onClick={() => setSelectedProject(project.name)}
            >
                {project.name}
            </button>
            {active ? jobs_elements : null}
        </div>
    );
}

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

    let project_elements;
    if (projects !== null) {
        project_elements = projects.map((project) => (
            <SidebarEntry
                key={project.name}
                project={project}
                jobs={jobs}
                selectedProject={selectedProject}
                setSelectedProject={setSelectedProject}
                selectedJob={selectedJob}
                setSelectedJob={setSelectedJob}
            />
        ));
    } else {
        project_elements = [1, 2, 3].map((project) => (
            <div key={project} className="px-3 py-2">
                <Skeleton variant="text" className="h-5 w-full" />
            </div>
        ));
    }

    useEffect(() => {
        setSelectedJob(null);
        setJobs(null);
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
            <h2
                className="mt-1 mb-4 pb-3.5 text-center text-[15px] font-semibold tracking-[0.01em] text-[#f2f2f2] border-b border-solid border-[#2e2e2e]
                "
            >
                Chariot Build Server
            </h2>
            <div>{project_elements}</div>
        </div>
    );
}

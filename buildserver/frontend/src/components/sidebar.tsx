import { useEffect, useState } from "preact/hooks";
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

    let jobs_elements;
    if (jobs !== null) {
        jobs_elements = jobs.toReversed().map((job) => (
            <button
                key={job}
                onClick={() => setSelectedJob(job)}
                className={cn(
                    "cursor-pointer truncate rounded-md px-2.5 py-2 text-left text-[15px] text-muted transition-colors hover:bg-elevated hover:text-fg",
                    selectedJob === job && "bg-elevated text-fg",
                )}
            >
                {job}
            </button>
        ));
    } else {
        jobs_elements = [1, 2, 3].map((job) => (
            <div key={job} className="px-2.5 py-2">
                <Skeleton variant="text" className="h-5 w-full" />
            </div>
        ));
    }

    useEffect(() => {
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
        <div className="flex min-h-screen w-64 shrink-0 flex-col gap-6 bg-panel px-3 py-5">
            <div className="flex flex-col gap-3">
                <span className="px-2.5 text-base font-semibold text-fg">
                    Chariot Build
                </span>
                <div className="h-0.5 shrink-0 bg-line" />
            </div>

            <div className="flex flex-col gap-1">
                <span className="px-2.5 pb-1 text-sm text-muted">Projects</span>
                {projects === null
                    ? [1, 2, 3].map((item) => (
                          <div key={item} className="px-2.5 py-2">
                              <Skeleton variant="text" className="h-5 w-full" />
                          </div>
                      ))
                    : projects.map((project) => (
                          <button
                              key={project.name}
                              onClick={() => setSelectedProject(project.name)}
                              className={cn(
                                  "cursor-pointer truncate rounded-md px-2.5 py-2 text-left text-[15px] text-muted transition-colors hover:bg-elevated hover:text-fg",
                                  selectedProject === project.name &&
                                      "bg-elevated text-fg",
                              )}
                          >
                              {project.name}
                          </button>
                      ))}
            </div>

            {selectedProject !== null && (
                <>
                    <div className="h-0.5 shrink-0 bg-line" />
                    <div className="flex min-h-0 flex-1 flex-col gap-1">
                        <span className="px-2.5 pb-1 text-sm text-muted">
                            Jobs
                        </span>
                        <div className="flex flex-col gap-1 overflow-y-auto">
                            {jobs_elements}
                        </div>
                    </div>
                </>
            )}
        </div>
    );
}

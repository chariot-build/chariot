import { useEffect, useState } from "preact/hooks";
import { Sidebar } from "./components/sidebar";
import { fetchJson } from "./utils/fetch";
import { JobDetails } from "./components/jobDetails";
import { useBuildEvents } from "./utils/events";
import { navigate, parseRoute, routePath, usePathname } from "./utils/router";

export type Project = {
    name: string;
    repository: string;
};

export function App() {
    let [projects, setProjects] = useState<Project[] | null>(null);
    let [refreshToken, setRefreshToken] = useState(0);

    const route = parseRoute(usePathname());
    const selectedProject = route.project;
    const selectedJob = route.job;

    const setSelectedProject = (value: string | null) =>
        navigate(routePath({ project: value, job: null, task: null }));

    const setSelectedJob = (value: string | null) =>
        navigate(routePath({ project: route.project, job: value, task: null }));

    useEffect(() => {
        const fetchData = async () => {
            try {
                const result = await fetchJson("/projects");
                setProjects(result.projects);
            } catch (err) {
                console.error("error");
            }
        };

        fetchData();
    }, []);

    useBuildEvents((event) => {
        if (
            event.type === "job_start" ||
            event.type === "job_end" ||
            event.type === "resync"
        ) {
            setRefreshToken((token) => token + 1);
        }
    });

    return (
        <>
            <Sidebar
                projects={projects}
                selectedProject={selectedProject}
                setSelectedProject={setSelectedProject}
                selectedJob={selectedJob}
                setSelectedJob={setSelectedJob}
                refreshToken={refreshToken}
            />
            <div className="flex-1 overflow-y-auto px-8 py-6">
                {selectedProject !== null && (
                    <div className="mb-6 flex items-center gap-2 text-base">
                        <span className="text-muted">{selectedProject}</span>
                        {selectedJob !== null && (
                            <>
                                <span className="text-muted">/</span>
                                <span className="font-medium text-fg">
                                    {selectedJob}
                                </span>
                            </>
                        )}
                    </div>
                )}
                {selectedJob !== null && (
                    <JobDetails
                        selectedJob={selectedJob}
                        refreshToken={refreshToken}
                    />
                )}
            </div>
        </>
    );
}

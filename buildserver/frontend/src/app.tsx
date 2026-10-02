import { useEffect, useState } from "preact/hooks";
import { Sidebar } from "./components/sidebar";
import { fetchJson } from "./utils/fetch";
import { JobDetails } from "./components/jobDetails";
import { useBuildEvents } from "./utils/events";

export type Project = {
    name: string;
    repository: string;
};

export function App() {
    let [projects, setProjects] = useState<Project[] | null>(null);
    let [selectedProject, setSelectedProject] = useState<string | null>(null);
    let [selectedJob, setSelectedJob] = useState<string | null>(null);
    let [refreshToken, setRefreshToken] = useState(0);

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
            <div className="flex-1 px-10 py-8 overflow-y-auto">
                <h2 className="m-0 mb-5 text-xl font-semibold text-[#f2f2f2]">
                    {selectedProject}{" "}
                    {selectedJob !== null ? `- ${selectedJob}` : ""}
                </h2>
                {selectedJob !== null ? (
                    <JobDetails
                        selectedProject={selectedProject}
                        selectedJob={selectedJob}
                        refreshToken={refreshToken}
                    />
                ) : (
                    <></>
                )}
            </div>
        </>
    );
}

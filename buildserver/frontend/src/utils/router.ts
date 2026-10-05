import { useEffect, useState } from "preact/hooks";

export type Route = {
    project: string | null;
    job: string | null;
    task: string | null;
};

const listeners = new Set<() => void>();

function notify() {
    for (const listener of listeners) {
        listener();
    }
}

export function navigate(to: string) {
    if (to === `${window.location.pathname}${window.location.search}`) {
        return;
    }

    window.history.pushState(null, "", to);
    notify();
}

export function usePathname() {
    const [pathname, setPathname] = useState(window.location.pathname);

    useEffect(() => {
        const update = () => setPathname(window.location.pathname);

        listeners.add(update);
        window.addEventListener("popstate", update);

        return () => {
            listeners.delete(update);
            window.removeEventListener("popstate", update);
        };
    }, []);

    return pathname;
}

export function parseRoute(pathname: string): Route {
    const segments = pathname.split("/").filter(Boolean);

    let project: string | null = null;
    let job: string | null = null;
    let task: string | null = null;

    if (segments[0] === "project" && segments[1] !== undefined) {
        project = decodeURIComponent(segments[1]);

        if (segments[2] === "job" && segments[3] !== undefined) {
            job = decodeURIComponent(segments[3]);

            if (segments[4] === "task" && segments[5] !== undefined) {
                task = decodeURIComponent(segments[5]);
            }
        }
    }

    return { project, job, task };
}

export function routePath(route: Route): string {
    if (route.project === null) {
        return "/";
    }

    let path = `/project/${encodeURIComponent(route.project)}`;

    if (route.job !== null) {
        path += `/job/${encodeURIComponent(route.job)}`;
    }

    if (route.task !== null) {
        path += `/task/${encodeURIComponent(route.task)}`;
    }

    return path;
}

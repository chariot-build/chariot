export type UrlState = {
    project: string | null;
    job: string | null;
    task: string | null;
    path: string | null;
};

export function readUrlState(): UrlState {
    const params = new URLSearchParams(window.location.search);

    return {
        project: params.get("project"),
        job: params.get("job"),
        task: params.get("task"),
        path: params.get("path"),
    };
}

export function updateUrlState(update: Partial<UrlState>) {
    const params = new URLSearchParams(window.location.search);

    for (const [key, value] of Object.entries(update)) {
        if (value === null) {
            params.delete(key);
        } else {
            params.set(key, value);
        }
    }

    const query = params.toString();
    window.history.replaceState(
        null,
        "",
        query ? `?${query}` : window.location.pathname,
    );
}

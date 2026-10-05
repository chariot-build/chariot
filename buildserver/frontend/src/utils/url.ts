export function readPath(): string | null {
    return new URLSearchParams(window.location.search).get("path");
}

export function writePath(path: string | null) {
    const params = new URLSearchParams(window.location.search);

    if (path === null) {
        params.delete("path");
    } else {
        params.set("path", path);
    }

    const query = params.toString();
    const url = query
        ? `${window.location.pathname}?${query}`
        : window.location.pathname;

    window.history.replaceState(null, "", url);
}

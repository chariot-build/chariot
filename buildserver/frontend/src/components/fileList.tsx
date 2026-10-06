import { useEffect, useRef, useState } from "preact/hooks";
import { File, Folder } from "lucide-preact";
import { API_BASE, fetchJson } from "../utils/fetch";
import { cn } from "cn";
import { readPath, writePath } from "../utils/url";

type File = {
    name: string;
    type: "directory" | "file";
};

function FileTypeIcon({ status }: { status: File["type"] }) {
    const base = "size-5 shrink-0";

    switch (status) {
        case "directory":
            return <Folder className={`${base} text-accent`} />;
        case "file":
            return <File className={`${base} text-muted`} />;
    }
}

function storePath(hash: string, path: string) {
    return path === "/" ? `/install-${hash}` : `/install-${hash}${path}`;
}

function downloadFile(hash: string, path: string, name: string) {
    const relative = `${storePath(hash, path)}/${encodeURIComponent(name)}`.replace(
        /^\//,
        "",
    );
    const link = document.createElement("a");
    link.href = `${API_BASE}/store/g/${relative}`;
    link.download = name;
    document.body.appendChild(link);
    link.click();
    link.remove();
}

function FileRow({
    file,
    hash,
    path,
    setPath,
}: {
    file: File;
    hash: string;
    path: string;
    setPath: (path: string) => void;
}) {
    return (
        <button
            onClick={() => {
                if (file.type === "directory") {
                    const segment = encodeURIComponent(file.name);
                    setPath(path === "/" ? `/${segment}` : `${path}/${segment}`);
                } else if (file.type === "file") {
                    downloadFile(hash, path, file.name);
                }
            }}
            className="flex w-full cursor-pointer items-center gap-3 rounded-md px-4 py-3 text-left transition-colors hover:bg-elevated"
        >
            <FileTypeIcon status={file.type} />
            <span className="min-w-0 flex-1 truncate text-base text-fg">
                {file.name}
            </span>
        </button>
    );
}

function Breadcrumbs({
    path,
    setPath,
}: {
    path: string;
    setPath: (path: string) => void;
}) {
    const segments = path.split("/").filter(Boolean);

    return (
        <div className="flex flex-wrap items-center gap-1 px-4 pb-3 text-base">
            <button
                onClick={() => setPath("/")}
                disabled={segments.length === 0}
                className={cn(
                    "rounded px-1 text-muted transition-colors",
                    segments.length > 0 && "cursor-pointer hover:text-fg",
                    segments.length === 0 && "text-fg",
                )}
            >
                /
            </button>
            {segments.map((segment, index) => {
                const isLast = index === segments.length - 1;
                const target = "/" + segments.slice(0, index + 1).join("/");

                return (
                    <span key={target} className="flex items-center gap-1">
                        {index > 0 && <span className="text-muted">/</span>}
                        <button
                            onClick={() => setPath(target)}
                            disabled={isLast}
                            className={cn(
                                "rounded px-1 text-muted transition-colors",
                                !isLast && "cursor-pointer hover:text-fg",
                                isLast && "text-fg",
                            )}
                        >
                            {decodeURIComponent(segment)}
                        </button>
                    </span>
                );
            })}
        </div>
    );
}

export function FileList({ hash }: { hash: string }) {
    let [path, setPath] = useState<string>("/");
    let [files, setFiles] = useState<File[]>([]);
    let firstLoad = useRef(true);

    useEffect(() => {
        if (firstLoad.current) {
            firstLoad.current = false;

            const stored = readPath();
            if (stored !== null && stored.startsWith("/")) {
                setPath(stored);
                return;
            }
        }

        setPath("/");
    }, [hash]);

    useEffect(() => {
        writePath(path);
    }, [path]);

    useEffect(() => {
        setFiles([]);

        const fetchData = async () => {
            try {
                const result = await fetchJson(
                    `/store/index?path=${storePath(hash, path)}`,
                );
                setFiles(result.entries);
            } catch (err) {
                console.error("error");
            }
        };

        fetchData();
    }, [hash, path]);

    return (
        <div className="flex w-full min-w-0 flex-col">
            <Breadcrumbs path={path} setPath={setPath} />
            <div className="flex flex-col gap-1">
                {files.map((file) => (
                    <FileRow
                        key={file.name}
                        file={file}
                        hash={hash}
                        path={path}
                        setPath={setPath}
                    />
                ))}
            </div>
        </div>
    );
}

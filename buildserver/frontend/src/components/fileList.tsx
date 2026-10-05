import { useEffect, useState } from "preact/hooks";
import { API_BASE, fetchJson } from "../utils/fetch";
import { cn } from "cn";

type File = {
    name: string;
    type: "directory" | "file";
};

function FileTypeIcon({ status }: { status: File["type"] }) {
    const base = "size-4 shrink-0";

    switch (status) {
        case "directory":
            return (
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} text-[#5b8cff]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                >
                    <path d="M4 7.5A2.5 2.5 0 0 1 6.5 5h3l2 2h6A2.5 2.5 0 0 1 20 9.5v7A2.5 2.5 0 0 1 17.5 19h-11A2.5 2.5 0 0 1 4 16.5Z" />
                </svg>
            );
        case "file":
            return (
                <svg
                    viewBox="0 0 24 24"
                    className={`${base} text-[#9a9a9a]`}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                >
                    <path d="M14 4H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z" />
                    <path d="M14 4v4h4" />
                </svg>
            );
    }
}

function downloadFile(path: string, name: string) {
    const relative = `${path}/${encodeURIComponent(name)}`.replace(/^\//, "");
    const link = document.createElement("a");
    link.href = `${API_BASE}/store/g/${relative}`;
    link.download = name;
    document.body.appendChild(link);
    link.click();
    link.remove();
}

function FileTableEntry({
    file,
    path,
    setPath,
}: {
    file: File;
    path: string;
    setPath: (path: string) => void;
}) {
    return (
        <tr
            onClick={() => {
                if (file.type === "directory") {
                    setPath(`${path}/${encodeURIComponent(file.name)}`);
                } else if (file.type === "file") {
                    downloadFile(path, file.name);
                }
            }}
            className="border-b border-[#2a2a2a] last:border-b-0 cursor-pointer hover:bg-[#242424]"
        >
            <td className="w-12 px-4 py-2.5">
                <FileTypeIcon status={file.type} />
            </td>
            <td className="px-4 py-2.5 text-sm text-[#c9c9c9]">
                {file.name}
            </td>
        </tr>
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
        <div className="flex flex-wrap items-center gap-1 border-b border-[#2a2a2a] bg-[#1c1c1c] px-4 py-2.5 text-sm">
            {segments.map((segment, index) => {
                const isLast = index === segments.length - 1;
                const target =
                    "/" + segments.slice(0, index + 1).join("/");

                return (
                    <span key={target} className="flex items-center gap-1">
                        {index > 1 && (
                            <span className="text-[#5a5a5a]">/</span>
                        )}
                        <button
                            onClick={() => setPath(target)}
                            disabled={isLast}
                            className={cn(
                                "rounded px-1 py-0.5 text-[#c9c9c9]",
                                !isLast &&
                                    "cursor-pointer hover:bg-[#2a2a2a] hover:text-white",
                                isLast && "text-[#f2f2f2] font-medium",
                            )}
                        >
                            {index === 0
                                ? "/"
                                : decodeURIComponent(segment)}
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

    useEffect(() => {
        setPath(`/install-${hash}`);
    }, [hash]);

    useEffect(() => {
        setFiles([]);

        const fetchData = async () => {
            try {
                const result = await fetchJson(`/store/index?path=${path}`);
                console.log(result);
                setFiles(result.entries);
            } catch (err) {
                console.error("error");
            }
        };

        fetchData();
    }, [path]);

    return (
        <div className="w-full min-w-0 border border-[#2e2e2e] rounded-lg overflow-hidden bg-[#1c1c1c]">
            <Breadcrumbs path={path} setPath={setPath} />
            <table className="w-full min-w-0 border-collapse">
                <tbody>
                    <tr className="border-b border-[#2a2a2a] last:border-b-0">
                        <td className="w-12 px-4 py-2.5 text-xs font-medium uppercase tracking-[0.03em] text-[#9a9a9a]">
                            Type
                        </td>
                        <td className="px-4 py-2.5 text-xs font-medium uppercase tracking-[0.03em] text-[#9a9a9a]">
                            Name
                        </td>
                    </tr>

                    {files.map((file) => (
                        <FileTableEntry
                            key={file.name}
                            file={file}
                            path={path}
                            setPath={setPath}
                        />
                    ))}
                </tbody>
            </table>
        </div>
    );
}

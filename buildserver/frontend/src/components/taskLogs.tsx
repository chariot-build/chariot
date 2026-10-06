import { cn } from "cn";

export function TaskLogs({
    id,
    name,
    type,
    status,
}: {
    id: number;
    name: string;
    type: string;
    status: string;
}) {
    const lines = [];
    for (let i = 0; i < 100; i++) {
        lines.push("meow");
    }

    return (
        <div className="flex w-full min-w-0 flex-col overflow-hidden rounded-md border border-line bg-panel font-mono text-sm">
            <div className="flex items-center justify-between border-b border-line px-4 py-2 text-xs text-muted">
                <span>{type}.log</span>
                <span>
                    {lines.length} {lines.length === 1 ? "line" : "lines"}
                </span>
            </div>
            <div className="max-h-[60vh] overflow-auto py-1">
                {lines.map((line, index) => (
                    <div
                        key={index}
                        className="flex gap-4 px-4 py-0.5 hover:bg-elevated"
                    >
                        <span className="w-6 shrink-0 select-none text-right text-muted/50">
                            {index + 1}
                        </span>
                        <span className={cn("whitespace-pre", "text-white")}>
                            {line}
                        </span>
                    </div>
                ))}
            </div>
        </div>
    );
}

import { useEffect, useRef } from "preact/hooks";
import { API_BASE } from "./fetch";

export type BuildServerEvent =
    | { type: "job_start"; jobId: number }
    | { type: "job_end" }
    | {
          type: "task_register";
          jobId: number;
          taskId: number;
          taskType: string;
          name: string;
      }
    | { type: "task_status"; jobId: number; taskId: number; status: string }
    | { type: "resync" };

type BuildServerEventHandler = (event: BuildServerEvent) => void;

const EVENT_TYPES = [
    "job_start",
    "job_end",
    "task_register",
    "task_status",
    "resync",
] as const;

const handlers = new Set<BuildServerEventHandler>();
let source: EventSource | null = null;

// Only one job can run at a time, so task events are implicitly scoped to the
// job announced by the most recent `job_start`.
let currentJobId: number | null = null;

function parseEvent(event: MessageEvent): BuildServerEvent | null {
    const data = event.data ? JSON.parse(event.data) : null;

    switch (event.type) {
        case "job_start":
            currentJobId = data.id;
            return { type: "job_start", jobId: data.id };
        case "job_end":
            currentJobId = null;
            return { type: "job_end" };
        case "task_register":
            if (currentJobId === null) {
                return null;
            }
            return {
                type: "task_register",
                jobId: currentJobId,
                taskId: data.id,
                taskType: data.type,
                name: data.name,
            };
        case "task_status":
            if (currentJobId === null) {
                return null;
            }
            return {
                type: "task_status",
                jobId: currentJobId,
                taskId: data.id,
                status: data.status,
            };
        case "resync":
            return { type: "resync" };
        default:
            return null;
    }
}

function dispatch(event: BuildServerEvent) {
    for (const handler of handlers) {
        handler(event);
    }
}

function getEventSource() {
    if (source !== null) {
        return source;
    }

    source = new EventSource(`${API_BASE}/events`);

    for (const eventType of EVENT_TYPES) {
        source.addEventListener(eventType, (event) => {
            const parsed = parseEvent(event);
            if (parsed !== null) {
                dispatch(parsed);
            }
        });
    }

    source.addEventListener("open", () => {
        dispatch({ type: "resync" });
    });

    return source;
}

export function subscribeToBuildEvents(handler: BuildServerEventHandler) {
    getEventSource();
    handlers.add(handler);

    return () => {
        handlers.delete(handler);
    };
}

export function useBuildEvents(handler: BuildServerEventHandler) {
    const handlerRef = useRef(handler);
    handlerRef.current = handler;

    useEffect(
        () => subscribeToBuildEvents((event) => handlerRef.current(event)),
        [],
    );
}

import { cn } from "cn";

export default function Skeleton({
    variant = "text",
    className,
}: {
    variant?: "text" | "circular" | "rectangular";
    className?: string;
}) {
    return (
        <div
            className={cn(
                "block bg-skeleton animate-skeleton",
                variant === "circular" ? "rounded-full" : "rounded",
                className,
            )}
        />
    );
}

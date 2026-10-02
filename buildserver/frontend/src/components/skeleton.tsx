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
                "block bg-[#a0a0a0] animate-skeleton",
                variant === "circular" ? "rounded-full" : "rounded",
                className,
            )}
        />
    );
}

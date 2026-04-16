import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

export function getStatusBadge(status: string) {
    const styles: Record<string, string> = {
        queued: "bg-helios-slate/10 text-helios-slate border-helios-slate/20",
        analyzing: "bg-blue-500/10 text-blue-500 border-blue-500/20",
        encoding: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
        remuxing: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
        completed: "bg-green-500/10 text-green-500 border-green-500/20",
        failed: "bg-red-500/10 text-red-500 border-red-500/20",
        cancelled: "bg-red-500/10 text-red-500 border-red-500/20",
        skipped: "bg-gray-500/10 text-gray-500 border-gray-500/20",
        archived: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20",
        resuming: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
    };

    return (
        <span
            className={cn(
                "px-2.5 py-1 rounded-md text-xs font-medium border capitalize",
                styles[status] || styles.queued,
            )}
        >
            {status}
        </span>
    );
}

interface TimeDisplayProps {
    value: string | null | undefined;
    className?: string;
}

function formatRelativeTime(date: Date): string {
    const diffMs = Math.max(0, Date.now() - date.getTime());
    const minutes = Math.floor(diffMs / 60_000);
    if (minutes < 1) return "Just now";
    if (minutes < 60) return `${minutes}m ago`;

    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;

    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d ago`;

    const months = Math.floor(days / 30);
    if (months < 12) return `${months}mo ago`;

    const years = Math.floor(days / 365);
    return `${years}y ago`;
}

export default function TimeDisplay({ value, className }: TimeDisplayProps) {
    if (!value) {
        return <span className={className}>Unknown time</span>;
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
        return <span className={className}>{value}</span>;
    }

    const local = date.toLocaleString();
    const utc = date.toISOString();
    const relative = formatRelativeTime(date);

    return (
        <time
            dateTime={utc}
            title={`Local: ${local}\nUTC: ${utc}`}
            aria-label={`${relative}. Local: ${local}. UTC: ${utc}`}
            tabIndex={0}
            className={className}
        >
            {relative}
        </time>
    );
}

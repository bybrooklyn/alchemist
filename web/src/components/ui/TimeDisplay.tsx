import { formatRelativeTime } from "../../lib/format";

interface TimeDisplayProps {
    value: string | null | undefined;
    className?: string;
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

interface RangeControlProps {
    label: string;
    min: number;
    max: number;
    step: number;
    value: number;
    onChange: (value: number) => void;
}

interface LabeledInputProps {
    label: string;
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    type?: string;
}

interface LabeledSelectProps {
    label: string;
    value: string;
    onChange: (value: string) => void;
    options: Array<{ value: string; label: string }>;
}

interface ToggleRowProps {
    title: string;
    body: string;
    checked: boolean;
    onChange: (checked: boolean) => void;
}

interface ReviewCardProps {
    title: string;
    lines: string[];
}

export function RangeControl({ label, min, max, step, value, onChange }: RangeControlProps) {
    return (
        <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">{label}</label>
            <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(parseFloat(e.target.value))} className="w-full accent-helios-solar" />
            <div className="text-sm font-semibold text-helios-ink">{value}</div>
        </div>
    );
}

export function LabeledInput({ label, value, onChange, placeholder, type = "text" }: LabeledInputProps) {
    return (
        <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">{label}</label>
            <input
                type={type}
                value={value}
                onChange={(e) => onChange(e.target.value)}
                placeholder={placeholder}
                className="w-full rounded-xl border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
            />
        </div>
    );
}

export function LabeledSelect({ label, value, onChange, options }: LabeledSelectProps) {
    return (
        <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">{label}</label>
            <select
                value={value}
                onChange={(e) => onChange(e.target.value)}
                className="w-full rounded-xl border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
            >
                {options.map((option) => (
                    <option key={option.value} value={option.value}>
                        {option.label}
                    </option>
                ))}
            </select>
        </div>
    );
}

export function ToggleRow({ title, body, checked, onChange }: ToggleRowProps) {
    return (
        <label className="flex items-center justify-between gap-4 rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4">
            <div>
                <p className="text-sm font-semibold text-helios-ink">{title}</p>
                <p className="text-xs text-helios-slate mt-1">{body}</p>
            </div>
            <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="h-5 w-5 rounded border-helios-line/30 accent-helios-solar" />
        </label>
    );
}

export function ReviewCard({ title, lines }: ReviewCardProps) {
    return (
        <div className="rounded-xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-5">
            <div className="text-sm font-semibold text-helios-ink">{title}</div>
            <div className="mt-3 space-y-2">
                {lines.map((line) => (
                    <p key={line} className="text-sm text-helios-slate break-words">
                        {line}
                    </p>
                ))}
            </div>
        </div>
    );
}

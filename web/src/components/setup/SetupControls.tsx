import { useId, type ReactNode } from "react";

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
    children?: ReactNode;
    disabled?: boolean;
}

interface ReviewCardProps {
    title: string;
    lines: string[];
}

export function RangeControl({ label, min, max, step, value, onChange }: RangeControlProps) {
    return (
        <div className="space-y-2">
            <label className="text-xs font-medium text-helios-slate">{label}</label>
            <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(parseFloat(e.target.value))} className="w-full accent-helios-solar h-1.5 cursor-pointer" />
            <div className="text-sm font-semibold text-helios-ink">{value}</div>
        </div>
    );
}

export function LabeledInput({ label, value, onChange, placeholder, type = "text" }: LabeledInputProps) {
    return (
        <div className="space-y-2">
            <label className="text-xs font-medium text-helios-slate">{label}</label>
            <input
                type={type}
                value={value}
                onChange={(e) => onChange(e.target.value)}
                placeholder={placeholder}
                className="w-full rounded-md border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
            />
        </div>
    );
}

export function LabeledSelect({ label, value, onChange, options }: LabeledSelectProps) {
    return (
        <div className="space-y-2">
            <label className="text-xs font-medium text-helios-slate">{label}</label>
            <select
                value={value}
                onChange={(e) => onChange(e.target.value)}
                className="w-full rounded-md border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
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

export function ToggleRow({ title, body, checked, onChange, children, disabled = false }: ToggleRowProps) {
    const inputId = useId();

    return (
        <div className={`rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4 ${disabled ? "opacity-70" : ""}`}>
            <div className="flex items-start justify-between gap-4">
                <label htmlFor={inputId} className={`block flex-1 ${disabled ? "cursor-not-allowed" : "cursor-pointer"}`}>
                    <p className="text-sm font-semibold text-helios-ink">{title}</p>
                    <p className="mt-1 text-xs text-helios-slate">{body}</p>
                </label>
                <input
                    id={inputId}
                    type="checkbox"
                    checked={checked}
                    onChange={(e) => onChange(e.target.checked)}
                    disabled={disabled}
                    className="mt-0.5 h-5 w-5 shrink-0 rounded border-helios-line/30 accent-helios-solar"
                />
            </div>
            {children && <div className="mt-3 border-t border-helios-line/10 pt-3">{children}</div>}
        </div>
    );
}

export function ReviewCard({ title, lines }: ReviewCardProps) {
    return (
        <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-5">
            <div className="text-xs font-medium text-helios-slate/70 pb-2 mb-2 border-b border-helios-line/20">
                {title}
            </div>
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

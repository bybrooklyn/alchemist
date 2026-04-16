import { Search, RefreshCw, ArrowDown, ArrowUp, Plus } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { RefObject } from "react";
import type React from "react";
import type { TabType, SortField } from "./types";
import { SORT_OPTIONS } from "./types";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface JobsToolbarProps {
    activeTab: TabType;
    setActiveTab: (tab: TabType) => void;
    setPage: (page: number) => void;
    searchInput: string;
    setSearchInput: (s: string) => void;
    compactSearchOpen: boolean;
    setCompactSearchOpen: (fn: boolean | ((prev: boolean) => boolean)) => void;
    compactSearchRef: RefObject<HTMLDivElement | null>;
    compactSearchInputRef: RefObject<HTMLInputElement | null>;
    sortBy: SortField;
    setSortBy: (s: SortField) => void;
    sortDesc: boolean;
    setSortDesc: (fn: boolean | ((prev: boolean) => boolean)) => void;
    refreshing: boolean;
    fetchJobs: () => Promise<void>;
    openEnqueueDialog: () => void;
}

export function JobsToolbar({
    activeTab, setActiveTab, setPage,
    searchInput, setSearchInput,
    compactSearchOpen, setCompactSearchOpen, compactSearchRef, compactSearchInputRef,
    sortBy, setSortBy, sortDesc, setSortDesc,
    refreshing, fetchJobs, openEnqueueDialog,
}: JobsToolbarProps) {
    return (
        <div className="rounded-xl border border-helios-line/10 bg-helios-surface/50 px-3 py-3">
            <div className="flex flex-wrap gap-1">
                {(["all", "active", "queued", "completed", "failed", "skipped", "archived"] as TabType[]).map((tab) => (
                    <button
                        key={tab}
                        onClick={() => { setActiveTab(tab); setPage(1); }}
                        className={cn(
                            "px-3 py-1.5 rounded-md text-sm font-medium transition-all capitalize sm:px-4",
                            activeTab === tab
                                ? "bg-helios-surface-soft text-helios-ink shadow-sm"
                                : "text-helios-slate hover:text-helios-ink"
                        )}
                    >
                        {tab}
                    </button>
                ))}
            </div>

            <div className="mt-3 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex items-center gap-2 sm:min-w-0 sm:flex-1">
                    <div className="relative hidden xl:block xl:w-64">
                        <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-helios-slate" size={14} />
                        <input
                            type="text"
                            placeholder="Search files..."
                            value={searchInput}
                            onChange={(e) => setSearchInput(e.target.value)}
                            className="w-full bg-helios-surface border border-helios-line/20 rounded-lg pl-9 pr-4 py-2 text-sm text-helios-ink focus:border-helios-solar outline-none"
                        />
                    </div>
                    <select
                        value={sortBy}
                        onChange={(e) => {
                            setSortBy(e.target.value as SortField);
                            setPage(1);
                        }}
                        className="h-10 min-w-0 flex-1 rounded-lg border border-helios-line/20 bg-helios-surface px-3 text-sm text-helios-ink outline-none focus:border-helios-solar sm:flex-none sm:w-44"
                    >
                        {SORT_OPTIONS.map((option) => (
                            <option key={option.value} value={option.value}>
                                {option.label}
                            </option>
                        ))}
                    </select>
                    <button
                        onClick={() => {
                            setSortDesc((current) => !current);
                            setPage(1);
                        }}
                        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-helios-line/20 bg-helios-surface text-helios-ink hover:bg-helios-surface-soft"
                        title={sortDesc ? "Sort descending" : "Sort ascending"}
                        aria-label={sortDesc ? "Sort descending" : "Sort ascending"}
                    >
                        {sortDesc ? <ArrowDown size={16} /> : <ArrowUp size={16} />}
                    </button>
                </div>

                <div className="flex items-center gap-2 sm:ml-auto">
                    <button
                        onClick={openEnqueueDialog}
                        className="inline-flex h-10 items-center gap-2 rounded-lg border border-helios-line/20 bg-helios-surface px-3 text-sm font-semibold text-helios-ink hover:bg-helios-surface-soft"
                    >
                        <Plus size={16} />
                        <span>Add file</span>
                    </button>
                    <button
                        onClick={() => void fetchJobs()}
                        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-helios-line/20 bg-helios-surface text-helios-ink hover:bg-helios-surface-soft"
                        title="Refresh jobs"
                        aria-label="Refresh jobs"
                    >
                        <RefreshCw size={16} className={refreshing ? "animate-spin" : undefined} />
                    </button>
                    <div ref={compactSearchRef as React.RefObject<HTMLDivElement>} className="relative xl:hidden">
                        <button
                            type="button"
                            onClick={() => setCompactSearchOpen((open) => (searchInput.trim() ? true : !open))}
                            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-helios-line/20 bg-helios-surface text-helios-ink hover:bg-helios-surface-soft"
                            title="Search files"
                            aria-label="Search files"
                        >
                            <Search size={16} />
                        </button>
                        <div
                            className={cn(
                                "absolute right-0 top-0 z-20 overflow-hidden rounded-lg border border-helios-line/20 bg-helios-surface shadow-lg shadow-helios-main/20 transition-[width,opacity] duration-200 ease-out",
                                compactSearchOpen
                                    ? "w-[min(18rem,calc(100vw-2rem))] opacity-100"
                                    : "pointer-events-none w-10 opacity-0"
                            )}
                        >
                            <div className="flex h-10 items-center px-3">
                                <Search size={16} className="shrink-0 text-helios-slate" />
                                <input
                                    ref={compactSearchInputRef as React.RefObject<HTMLInputElement>}
                                    type="text"
                                    placeholder="Search files..."
                                    value={searchInput}
                                    onChange={(e) => setSearchInput(e.target.value)}
                                    className="ml-2 min-w-0 flex-1 bg-transparent text-sm text-helios-ink outline-none placeholder:text-helios-slate"
                                />
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}

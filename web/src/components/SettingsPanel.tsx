import { useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { FolderOpen, Bell, Calendar, FileCog, Cog, Server, LayoutGrid, Palette } from "lucide-react";
import WatchFolders from "./WatchFolders";
import NotificationSettings from "./NotificationSettings";
import ScheduleSettings from "./ScheduleSettings";
import FileSettings from "./FileSettings";
import TranscodeSettings from "./TranscodeSettings";
import SystemSettings from "./SystemSettings";
import HardwareSettings from "./HardwareSettings";
import AppearanceSettings from "./AppearanceSettings";

const TABS = [
    { id: "appearance", label: "Appearance", icon: Palette, component: AppearanceSettings },
    { id: "watch", label: "Watch Folders", icon: FolderOpen, component: WatchFolders },
    { id: "transcode", label: "Transcoding", icon: Cog, component: TranscodeSettings },
    { id: "files", label: "File Management", icon: FileCog, component: FileSettings },
    { id: "schedule", label: "Schedule", icon: Calendar, component: ScheduleSettings },
    { id: "notifications", label: "Notifications", icon: Bell, component: NotificationSettings },
    { id: "hardware", label: "Hardware", icon: LayoutGrid, component: HardwareSettings },
    { id: "system", label: "System", icon: Server, component: SystemSettings },
];

export default function SettingsPanel() {
    const [activeTab, setActiveTab] = useState(() => {
        if (typeof window === "undefined") return "watch";
        const params = new URLSearchParams(window.location.search);
        const requested = params.get("tab");
        return requested && TABS.some((tab) => tab.id === requested) ? requested : "watch";
    });
    const [[page, direction], setPage] = useState([0, 0]);

    const activeIndex = TABS.findIndex(t => t.id === activeTab);

    const paginate = (newTabId: string) => {
        const newIndex = TABS.findIndex(t => t.id === newTabId);
        const newDirection = newIndex > activeIndex ? 1 : -1;
        setPage([newIndex, newDirection]);
        setActiveTab(newTabId);
        if (typeof window !== "undefined") {
            const url = new URL(window.location.href);
            url.searchParams.set("tab", newTabId);
            window.history.replaceState({}, "", url.toString());
        }
    };

    const navItemRefs = useRef<Record<string, HTMLButtonElement | null>>({});

    useEffect(() => {
        if (activeIndex < 0) {
            setActiveTab("watch");
        }
    }, [activeIndex]);

    useEffect(() => {
        const target = navItemRefs.current[activeTab];
        if (target) {
            target.scrollIntoView({ block: "nearest" });
        }
    }, [activeTab]);

    const variants = {
        enter: (direction: number) => ({
            y: direction > 0 ? 20 : -20,
            opacity: 0
        }),
        center: {
            zIndex: 1,
            y: 0,
            opacity: 1
        },
        exit: (direction: number) => ({
            zIndex: 0,
            y: direction < 0 ? 20 : -20,
            opacity: 0
        })
    };

    return (
        <div className="flex flex-col lg:flex-row gap-8">
            {/* Sidebar Navigation for Settings */}
            <nav className="w-full lg:w-64 flex-shrink-0">
                <div className="sticky top-8 max-h-[calc(100vh-8rem)] overflow-y-auto pr-1 space-y-1">
                    {TABS.map((tab) => {
                        const isActive = activeTab === tab.id;
                        return (
                            <button
                                key={tab.id}
                                ref={(node) => {
                                    navItemRefs.current[tab.id] = node;
                                }}
                                onClick={() => paginate(tab.id)}
                                className={`w-full flex items-center gap-3 px-4 py-3 rounded-xl text-sm font-bold transition-all duration-200 relative overflow-hidden group ${isActive
                                    ? "text-helios-ink bg-helios-surface-soft shadow-sm border border-helios-line/20"
                                    : "text-helios-slate hover:text-helios-ink hover:bg-helios-surface-soft/50"
                                    }`}
                            >
                                {isActive && (
                                    <motion.div
                                        layoutId="active-tab"
                                        className="absolute inset-0 bg-helios-surface-soft border border-helios-line/20 rounded-xl"
                                        initial={false}
                                        transition={{ type: "spring", stiffness: 300, damping: 30 }}
                                    />
                                )}
                                <span className="relative z-10 flex items-center gap-3">
                                    <tab.icon size={18} className={isActive ? "text-helios-solar" : "opacity-70 group-hover:opacity-100"} />
                                    {tab.label}
                                </span>
                            </button>
                        );
                    })}
                </div>
            </nav>

            {/* Content Area */}
            <div className="flex-1 min-w-0">
                <AnimatePresence mode="wait" initial={false} custom={direction}>
                    <motion.div
                        key={activeTab}
                        custom={direction}
                        variants={variants}
                        initial="enter"
                        animate="center"
                        exit="exit"
                        transition={{
                            opacity: { duration: 0.15, ease: "easeInOut" }
                        }}
                        className="p-1" // minimal padding for focus rings
                    >
                        {/* 
                           We render the active component. 
                           Container styling is applied here to wrap the component uniformly.
                        */}
                        <div className="bg-helios-surface border border-helios-line/20 rounded-3xl p-6 sm:p-8 shadow-sm">
                            <div className="mb-6">
                                <h2 className="text-xl font-bold text-helios-ink flex items-center gap-2">
                                    {(() => {
                                        const tab = TABS.find((t) => t.id === activeTab);
                                        if (!tab) return null;
                                        return (
                                            <>
                                                <tab.icon size={22} className="text-helios-solar" />
                                                {tab.label}
                                            </>
                                        );
                                    })()}
                                </h2>
                            </div>
                            {(() => {
                                const TabComponent = TABS.find((t) => t.id === activeTab)?.component;
                                return TabComponent ? <TabComponent /> : null;
                            })()}
                        </div>
                    </motion.div>
                </AnimatePresence>
            </div>
        </div>
    );
}

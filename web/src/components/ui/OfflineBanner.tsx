import { WifiOff } from "lucide-react";
import { useOnlineStatus } from "../../lib/useOnlineStatus";

export default function OfflineBanner() {
    const online = useOnlineStatus();

    if (online) return null;

    return (
        <div
            role="alert"
            aria-live="assertive"
            className="fixed bottom-4 left-1/2 -translate-x-1/2 z-[400] flex items-center gap-2 rounded-lg border border-amber-500/35 bg-helios-surface/95 px-4 py-2.5 text-sm font-medium text-amber-500 shadow-xl backdrop-blur-xl"
        >
            <WifiOff size={16} />
            Can&apos;t reach the Alchemist server. Reconnecting…
        </div>
    );
}

import { useEffect } from "react";
import { apiFetch, apiJson } from "../lib/api";
import { stripBasePath, withBasePath } from "../lib/basePath";

interface SetupStatus {
    setup_required?: boolean;
}

export default function AuthGuard() {
    useEffect(() => {
        let cancelled = false;

        const checkAuth = async () => {
            const path = stripBasePath(window.location.pathname);
            const isAuthPage = path.startsWith("/login") || path.startsWith("/setup");
            if (isAuthPage) {
                return;
            }

            try {
                const engineStatus = await apiFetch("/api/engine/status");
                if (engineStatus.status !== 401 || cancelled) {
                    return;
                }

                const setupStatus = await apiJson<SetupStatus>("/api/setup/status");
                if (cancelled) {
                    return;
                }

                window.location.href = setupStatus.setup_required
                    ? withBasePath("/setup")
                    : withBasePath("/login");
            } catch {
                // Keep user on current page on transient backend/network failures.
            }
        };

        const handleAfterSwap = () => {
            void checkAuth();
        };

        void checkAuth();
        document.addEventListener("astro:after-swap", handleAfterSwap);

        return () => {
            cancelled = true;
            document.removeEventListener("astro:after-swap", handleAfterSwap);
        };
    }, []);

    return null;
}

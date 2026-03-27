export type ToastKind = "success" | "error" | "info" | "warning";

export interface ToastInput {
    kind: ToastKind;
    message: string;
    title?: string;
    durationMs?: number;
}

export interface ToastMessage extends ToastInput {
    id: string;
}

const TOAST_EVENT = "alchemist:toast";

function nextToastId(): string {
    return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function showToast(input: ToastInput): void {
    if (typeof window === "undefined") {
        return;
    }

    const payload: ToastMessage = {
        ...input,
        id: nextToastId(),
    };

    window.dispatchEvent(
        new CustomEvent<ToastMessage>(TOAST_EVENT, {
            detail: payload,
        })
    );
}

export function subscribeToToasts(callback: (message: ToastMessage) => void): () => void {
    if (typeof window === "undefined") {
        return () => undefined;
    }

    const handler = (event: Event) => {
        const customEvent = event as CustomEvent<ToastMessage>;
        if (!customEvent.detail) {
            return;
        }
        callback(customEvent.detail);
    };

    window.addEventListener(TOAST_EVENT, handler as EventListener);
    return () => window.removeEventListener(TOAST_EVENT, handler as EventListener);
}

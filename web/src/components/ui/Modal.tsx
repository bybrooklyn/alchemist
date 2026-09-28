import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { motion, AnimatePresence, type Target } from "framer-motion";
import { cn } from "../../lib/cn";
import { focusableElements, restoreFocus, setAppShellInert, trapTabNavigation } from "../../lib/focusUtils";

function originInitial(originRect: DOMRect): Target {
    const buttonCenterX = originRect.left + originRect.width / 2;
    const buttonCenterY = originRect.top + originRect.height / 2;
    const viewportCenterX = window.innerWidth / 2;
    const viewportCenterY = window.innerHeight / 2;
    return {
        opacity: 0,
        scale: 0.4,
        x: buttonCenterX - viewportCenterX,
        y: buttonCenterY - viewportCenterY,
    };
}

interface ModalProps {
    open: boolean;
    onClose: () => void;
    labelledBy: string;
    describedBy?: string;
    maxWidth?: string;
    panelClassName?: string;
    /** Set false when a caller already owns focus-trapping (e.g. layered dialogs that must coordinate Escape handling). */
    trapFocus?: boolean;
    /** Caller-owned panel ref. Implies the caller manages its own focus trap; Modal skips its internal one. */
    panelRef?: RefObject<HTMLDivElement | null>;
    /** Element to focus on open instead of the first focusable child. */
    initialFocusRef?: RefObject<HTMLElement | null>;
    closeOnBackdrop?: boolean;
    /** Suppresses Escape/backdrop close, e.g. while a submit is in flight. */
    disableClose?: boolean;
    /** Zooms the panel in from a trigger element's screen position instead of a plain fade/scale. */
    originRect?: DOMRect | null;
    /** Backdrop + panel wrapper z-index. Panel content sits one layer above. Raise for dialogs meant to stack above another open Modal. */
    zIndexBase?: number;
    children: ReactNode;
}

export default function Modal({
    open,
    onClose,
    labelledBy,
    describedBy,
    maxWidth = "max-w-lg",
    panelClassName,
    trapFocus = true,
    panelRef: externalPanelRef,
    initialFocusRef,
    closeOnBackdrop = true,
    disableClose = false,
    originRect = null,
    zIndexBase = 100,
    children,
}: ModalProps) {
    const internalPanelRef = useRef<HTMLDivElement | null>(null);
    const panelRef = externalPanelRef ?? internalPanelRef;
    const lastFocusedRef = useRef<HTMLElement | null>(null);
    const [mounted, setMounted] = useState(false);
    const manageFocus = trapFocus && !externalPanelRef;

    // Callers routinely pass inline arrows (and toggle `disableClose` while
    // submits are in flight). Keeping them in refs lets the focus lifecycle
    // below run strictly on open/close instead of churning — teardown steals
    // focus back to the trigger and re-focusses the panel's first element on
    // every parent render otherwise.
    const onCloseRef = useRef(onClose);
    useEffect(() => {
        onCloseRef.current = onClose;
    });
    const disableCloseRef = useRef(disableClose);
    useEffect(() => {
        disableCloseRef.current = disableClose;
    });

    useEffect(() => setMounted(true), []);

    useEffect(() => {
        if (!open || !manageFocus) {
            return;
        }

        setAppShellInert(true);
        lastFocusedRef.current = document.activeElement as HTMLElement | null;

        const panel = panelRef.current;
        if (initialFocusRef?.current) {
            initialFocusRef.current.focus();
        } else if (panel) {
            const focusables = focusableElements(panel);
            (focusables[0] ?? panel).focus();
        }

        const onKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.preventDefault();
                if (!disableCloseRef.current) {
                    onCloseRef.current();
                }
                return;
            }

            if (event.key !== "Tab") {
                return;
            }

            trapTabNavigation(event, panelRef.current);
        };

        document.addEventListener("keydown", onKeyDown);
        return () => {
            document.removeEventListener("keydown", onKeyDown);
            setAppShellInert(false);
            restoreFocus(lastFocusedRef);
        };
    }, [open, manageFocus]);

    if (!mounted) {
        return null;
    }

    const initial: Target = originRect
        ? originInitial(originRect)
        : { opacity: 0, scale: 0.95, y: 10 };

    return createPortal(
        <AnimatePresence>
            {open && (
                <>
                    <motion.div
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        onClick={() => closeOnBackdrop && !disableClose && onClose()}
                        className="fixed inset-0 bg-black/60 backdrop-blur-sm"
                        style={{ zIndex: zIndexBase }}
                    />
                    <div
                        className="fixed inset-0 flex items-center justify-center px-4 pointer-events-none"
                        style={{ zIndex: zIndexBase + 1 }}
                    >
                        <motion.div
                            ref={panelRef as RefObject<HTMLDivElement>}
                            initial={initial}
                            animate={{ opacity: 1, scale: 1, x: 0, y: 0 }}
                            exit={initial}
                            transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
                            onClick={(event) => event.stopPropagation()}
                            role="dialog"
                            aria-modal="true"
                            aria-labelledby={labelledBy}
                            aria-describedby={describedBy}
                            tabIndex={-1}
                            className={cn(
                                "w-full bg-helios-surface border border-helios-line/20 rounded-lg shadow-2xl pointer-events-auto overflow-hidden outline-none",
                                maxWidth,
                                panelClassName,
                            )}
                        >
                            {children}
                        </motion.div>
                    </div>
                </>
            )}
        </AnimatePresence>,
        document.body,
    );
}

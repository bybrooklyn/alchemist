const FOCUSABLE_SELECTOR = [
    "a[href]",
    "button:not([disabled])",
    "input:not([disabled])",
    "select:not([disabled])",
    "textarea:not([disabled])",
    "[tabindex]:not([tabindex='-1'])",
].join(",");

export function focusableElements(root: HTMLElement): HTMLElement[] {
    return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
        (element) => !element.hasAttribute("disabled")
    );
}

const APP_SHELL_SELECTOR = ".app-shell";

export function setAppShellInert(inert: boolean): void {
    const shells = document.querySelectorAll<HTMLElement>(APP_SHELL_SELECTOR);
    for (const shell of shells) {
        if (inert) {
            shell.setAttribute("inert", "");
        } else {
            shell.removeAttribute("inert");
        }
    }
}

/** Cycle Tab/Shift+Tab inside `root` (shared focus-trap behavior for Modal
 *  and the job-detail dialog). No-ops for non-Tab keys and a null root;
 *  moves focus to the panel itself when it has no focusable children. */
export function trapTabNavigation(event: KeyboardEvent, root: HTMLElement | null): void {
    if (event.key !== "Tab" || !root) {
        return;
    }

    const focusables = focusableElements(root);
    if (focusables.length === 0) {
        event.preventDefault();
        root.focus();
        return;
    }

    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const current = document.activeElement as HTMLElement | null;

    if (event.shiftKey && current === first) {
        event.preventDefault();
        last.focus();
    } else if (!event.shiftKey && current === last) {
        event.preventDefault();
        first.focus();
    }
}

/** Return focus to the element captured before a dialog opened, if any. */
export function restoreFocus(capturedRef: { current: HTMLElement | null }): void {
    capturedRef.current?.focus();
}

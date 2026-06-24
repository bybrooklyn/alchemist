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

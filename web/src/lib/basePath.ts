declare global {
    interface Window {
        __ALCHEMIST_BASE_URL__?: string;
    }
}

const PLACEHOLDER = "__ALCHEMIST_BASE_URL__";

function normalize(value: string | undefined): string {
    const raw = (value ?? "").trim();
    if (!raw || raw === "/" || raw === PLACEHOLDER) {
        return "";
    }
    return raw.replace(/\/+$/, "");
}

export function getBasePath(): string {
    if (typeof window !== "undefined") {
        return normalize(window.__ALCHEMIST_BASE_URL__);
    }
    return "";
}

export function withBasePath(path: string): string {
    if (/^[a-z]+:\/\//i.test(path)) {
        return path;
    }

    const basePath = getBasePath();
    if (!path) {
        return basePath || "/";
    }

    if (path.startsWith("/")) {
        return `${basePath}${path}`;
    }

    return `${basePath}/${path}`;
}

export function stripBasePath(pathname: string): string {
    const basePath = getBasePath();
    if (!basePath) {
        return pathname || "/";
    }
    if (pathname === basePath) {
        return "/";
    }
    if (pathname.startsWith(`${basePath}/`)) {
        return pathname.slice(basePath.length) || "/";
    }
    return pathname || "/";
}

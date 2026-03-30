export interface ApiErrorShape {
    status: number;
    message: string;
    body?: unknown;
    url: string;
}

export class ApiError extends Error implements ApiErrorShape {
    status: number;
    body?: unknown;
    url: string;

    constructor({ status, message, body, url }: ApiErrorShape) {
        super(message);
        this.name = "ApiError";
        this.status = status;
        this.body = body;
        this.url = url;
    }
}

function bodyMessage(body: unknown): string | null {
    if (typeof body === "string") {
        const trimmed = body.trim();
        return trimmed.length > 0 ? trimmed : null;
    }
    if (body && typeof body === "object") {
        const known = body as { message?: unknown; error?: unknown; detail?: unknown };
        if (typeof known.message === "string" && known.message.trim().length > 0) {
            return known.message;
        }
        if (typeof known.error === "string" && known.error.trim().length > 0) {
            return known.error;
        }
        if (typeof known.detail === "string" && known.detail.trim().length > 0) {
            return known.detail;
        }
    }
    return null;
}

async function parseResponseBody(response: Response): Promise<unknown> {
    if (response.status === 204 || response.status === 205) {
        return undefined;
    }

    const contentType = response.headers.get("content-type") ?? "";
    if (contentType.includes("application/json")) {
        return response.json();
    }

    const text = await response.text();
    if (text.length === 0) {
        return undefined;
    }

    try {
        return JSON.parse(text);
    } catch {
        return text;
    }
}

async function toApiError(url: string, response: Response): Promise<ApiError> {
    const body = await parseResponseBody(response).catch(() => undefined);
    const message =
        bodyMessage(body) ??
        response.statusText ??
        `Request failed with status ${response.status}`;
    return new ApiError({
        status: response.status,
        message,
        body,
        url,
    });
}

export function isApiError(error: unknown): error is ApiError {
    return error instanceof ApiError;
}

/**
 * Authenticated fetch utility using cookie auth.
 */
export async function apiFetch(url: string, options: RequestInit = {}): Promise<Response> {
    const headers = new Headers(options.headers);

    if (!headers.has("Content-Type") && typeof options.body === "string") {
        headers.set("Content-Type", "application/json");
    }

    const controller = new AbortController();
    // 30s timeout: hardware detection and large scans can take time
    const timeoutMs = 30000;
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

    const abortHandler = () => controller.abort();

    if (options.signal) {
        if (options.signal.aborted) {
            controller.abort();
        } else {
            options.signal.addEventListener("abort", abortHandler);
        }
    }

    try {
        const response = await fetch(url, {
            ...options,
            headers,
            credentials: options.credentials ?? "same-origin",
            signal: controller.signal,
        });

        if (response.status === 401 && typeof window !== "undefined") {
            const path = window.location.pathname;
            const isAuthPage = path.startsWith("/login") || path.startsWith("/setup");
            if (!isAuthPage) {
                window.location.href = "/login";
                return new Promise(() => {});
            }
        }

        return response;
    } finally {
        clearTimeout(timeoutId);
        if (options.signal) {
            options.signal.removeEventListener("abort", abortHandler);
        }
    }
}

export async function apiJson<T>(url: string, options: RequestInit = {}): Promise<T> {
    const response = await apiFetch(url, options);
    if (!response.ok) {
        throw await toApiError(url, response);
    }
    return (await parseResponseBody(response)) as T;
}

export async function apiAction(url: string, options: RequestInit = {}): Promise<void> {
    const response = await apiFetch(url, options);
    if (!response.ok) {
        throw await toApiError(url, response);
    }
}

/**
 * Helper for GET JSON requests.
 */
export async function apiGet<T>(url: string): Promise<T> {
    return apiJson<T>(url);
}

/**
 * Helper for POST JSON requests.
 */
export async function apiPost<T>(url: string, body?: unknown): Promise<T> {
    return apiJson<T>(url, {
        method: "POST",
        body: body ? JSON.stringify(body) : undefined,
    });
}

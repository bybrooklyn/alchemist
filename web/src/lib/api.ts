export interface ApiErrorShape {
    status: number;
    message: string;
    body?: unknown;
    url: string;
}

/** Pulls the machine-readable `code` from an RFC7807 problem+json body. */
function bodyCode(body: unknown): string | undefined {
    if (body && typeof body === "object") {
        const known = body as { code?: unknown; error?: { code?: unknown } };
        if (typeof known.code === "string" && known.code.length > 0) {
            return known.code;
        }
        if (typeof known.error?.code === "string" && known.error.code.length > 0) {
            return known.error.code;
        }
    }
    return undefined;
}

/** Pulls the `docs_url` from an RFC7807 problem+json body. */
function bodyDocsUrl(body: unknown): string | undefined {
    if (body && typeof body === "object") {
        const known = body as { docs_url?: unknown; error?: { docs_url?: unknown } };
        if (typeof known.docs_url === "string" && known.docs_url.length > 0) {
            return known.docs_url;
        }
        if (typeof known.error?.docs_url === "string" && known.error.docs_url.length > 0) {
            return known.error.docs_url;
        }
    }
    return undefined;
}

export class ApiError extends Error implements ApiErrorShape {
    status: number;
    body?: unknown;
    url: string;
    /** Stable error code from the API problem document, when present. */
    code?: string;
    /** Docs link for `code`, when the API supplied one. */
    docsUrl?: string;

    constructor({ status, message, body, url }: ApiErrorShape) {
        super(message);
        this.name = "ApiError";
        this.status = status;
        this.body = body;
        this.url = url;
        this.code = bodyCode(body);
        this.docsUrl = bodyDocsUrl(body);
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
        if (
            known.error &&
            typeof known.error === "object" &&
            typeof (known.error as { message?: unknown }).message === "string" &&
            (known.error as { message: string }).message.trim().length > 0
        ) {
            return (known.error as { message: string }).message;
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

/** A `RequestInit` plus an optional per-call timeout override. */
export interface ApiFetchOptions extends RequestInit {
    /**
     * Abort the request after this many milliseconds. Defaults to 30s.
     * Pass `null` to disable the timeout entirely — required for long-running
     * transfers like Convert uploads, where a fixed ceiling would abort a large
     * file mid-upload.
     */
    timeoutMs?: number | null;
}

const DEFAULT_TIMEOUT_MS = 30000;

/**
 * Authenticated fetch utility using cookie auth.
 */
export async function apiFetch(url: string, options: ApiFetchOptions = {}): Promise<Response> {
    const resolvedUrl = url;
    const { timeoutMs: timeoutOption, ...requestInit } = options;
    const headers = new Headers(requestInit.headers);

    if (!headers.has("Content-Type") && typeof requestInit.body === "string") {
        headers.set("Content-Type", "application/json");
    }

    const controller = new AbortController();
    // 30s default covers hardware detection and large scans; callers that stream
    // big bodies (uploads) pass `timeoutMs: null` to opt out.
    const timeoutMs = timeoutOption === undefined ? DEFAULT_TIMEOUT_MS : timeoutOption;
    const timeoutId =
        timeoutMs === null ? null : setTimeout(() => controller.abort(), timeoutMs);

    const abortHandler = () => controller.abort();

    if (options.signal) {
        if (options.signal.aborted) {
            controller.abort();
        } else {
            options.signal.addEventListener("abort", abortHandler);
        }
    }

    try {
        const response = await fetch(resolvedUrl, {
            ...requestInit,
            headers,
            credentials: requestInit.credentials ?? "same-origin",
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
        if (timeoutId !== null) {
            clearTimeout(timeoutId);
        }
        if (requestInit.signal) {
            requestInit.signal.removeEventListener("abort", abortHandler);
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

/**
 * Authenticated fetch utility - uses cookie auth by default and adds Bearer
 * token only if one is explicitly present in localStorage.
 */
export async function apiFetch(url: string, options: RequestInit = {}): Promise<Response> {
    let token: string | null = null;
    try {
        token = localStorage.getItem('alchemist_token');
    } catch {
        token = null;
    }

    const headers = new Headers(options.headers);

    if (token) {
        headers.set('Authorization', `Bearer ${token}`);
    }

    if (!headers.has('Content-Type') && typeof options.body === 'string') {
        headers.set('Content-Type', 'application/json');
    }

    const controller = new AbortController();
    const timeoutMs = 15000;
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

    if (options.signal) {
        if (options.signal.aborted) {
            controller.abort();
        } else {
            options.signal.addEventListener('abort', () => controller.abort(), { once: true });
        }
    }

    try {
        const response = await fetch(url, {
            ...options,
            headers,
            credentials: options.credentials ?? 'same-origin',
            signal: controller.signal,
        });

        if (response.status === 401) {
            try {
                localStorage.removeItem('alchemist_token');
            } catch {
                // Ignore storage failures (e.g., restricted environments)
            }
            if (typeof window !== 'undefined') {
                const path = window.location.pathname;
                const isAuthPage = path.startsWith('/login') || path.startsWith('/setup');
                if (!isAuthPage) {
                    window.location.href = '/login';
                }
            }
        }

        return response;
    } finally {
        clearTimeout(timeoutId);
    }
}

/**
 * Helper for GET requests
 */
export async function apiGet(url: string): Promise<Response> {
    return apiFetch(url);
}

/**
 * Helper for POST requests with JSON body
 */
export async function apiPost(url: string, body?: unknown): Promise<Response> {
    return apiFetch(url, {
        method: 'POST',
        body: body ? JSON.stringify(body) : undefined
    });
}

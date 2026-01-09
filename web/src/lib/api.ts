/**
 * Authenticated fetch utility - automatically adds Bearer token from localStorage
 */
export async function apiFetch(url: string, options: RequestInit = {}): Promise<Response> {
    const token = localStorage.getItem('alchemist_token');

    const headers = new Headers(options.headers);

    if (token) {
        headers.set('Authorization', `Bearer ${token}`);
    }

    if (!headers.has('Content-Type') && options.body) {
        headers.set('Content-Type', 'application/json');
    }

    return fetch(url, {
        ...options,
        headers
    });
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

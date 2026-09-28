// Secure-context-safe unique IDs. `crypto.randomUUID()` requires a secure
// context and crashes plain-HTTP deployments, while `Math.random()` is not
// a CSPRNG (Sonar S2245). `crypto.getRandomValues()` is the compliant
// middle ground: cryptographically strong and available on plain HTTP.

export function uniqueId(prefix = "id"): string {
    const bytes = new Uint32Array(2);
    crypto.getRandomValues(bytes);
    return `${prefix}-${Date.now()}-${bytes[0].toString(36)}${bytes[1].toString(36)}`;
}

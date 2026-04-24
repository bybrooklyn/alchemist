import { useEffect, useState } from "react";
import { KeyRound, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";

type ApiTokenAccessLevel = "read_only" | "full_access" | "arr_webhook";

interface ApiToken {
    id: number;
    name: string;
    access_level: ApiTokenAccessLevel;
    created_at: string;
    last_used_at: string | null;
    revoked_at: string | null;
}

interface CreatedApiTokenResponse {
    token: ApiToken;
    plaintext_token: string;
}

export default function ApiTokenSettings() {
    const [tokens, setTokens] = useState<ApiToken[]>([]);
    const [loading, setLoading] = useState(true);
    const [name, setName] = useState("");
    const [accessLevel, setAccessLevel] = useState<ApiTokenAccessLevel>("read_only");
    const [error, setError] = useState<string | null>(null);
    const [pendingDeleteId, setPendingDeleteId] = useState<number | null>(null);
    const [createdTokenValue, setCreatedTokenValue] = useState<string | null>(null);

    useEffect(() => {
        void fetchTokens();
    }, []);

    const fetchTokens = async () => {
        try {
            const data = await apiJson<ApiToken[]>("/api/settings/api-tokens");
            setTokens(data);
            setError(null);
        } catch (err) {
            setError(isApiError(err) ? err.message : "Failed to load API tokens.");
        } finally {
            setLoading(false);
        }
    };

    const handleCreate = async (event: React.FormEvent) => {
        event.preventDefault();
        try {
            const payload = await apiJson<CreatedApiTokenResponse>("/api/settings/api-tokens", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name,
                    access_level: accessLevel,
                }),
            });
            setTokens((current) => [payload.token, ...current]);
            setCreatedTokenValue(payload.plaintext_token);
            setName("");
            setAccessLevel("read_only");
            showToast({
                kind: "success",
                title: "API Tokens",
                message: "Token created. Copy it now — it will not be shown again.",
            });
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to create API token.";
            setError(message);
            showToast({ kind: "error", title: "API Tokens", message });
        }
    };

    const handleRevoke = async (id: number) => {
        try {
            await apiAction(`/api/settings/api-tokens/${id}`, { method: "DELETE" });
            setTokens((current) =>
                current.map((token) =>
                    token.id === id
                        ? { ...token, revoked_at: new Date().toISOString() }
                        : token,
                ),
            );
            showToast({
                kind: "success",
                title: "API Tokens",
                message: "Token revoked.",
            });
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to revoke token.";
            setError(message);
            showToast({ kind: "error", title: "API Tokens", message });
        }
    };

    return (
        <div className="space-y-6" aria-live="polite">
            <div className="rounded-xl border border-helios-line/20 bg-helios-surface-soft p-4">
                <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                    <ShieldCheck size={16} className="text-helios-solar" />
                    Static API Tokens
                </div>
                <p className="mt-1 text-xs text-helios-slate">
                    Read-only tokens are observability-only. ARR webhook tokens are limited to
                    <span className="mx-1 font-mono">POST /api/webhooks/arr</span>.
                    Full-access tokens can do everything an authenticated session can do.
                </p>
            </div>

            {error && (
                <div className="rounded-lg border border-status-error/20 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {error}
                </div>
            )}

            {createdTokenValue && (
                <div className="rounded-lg border border-helios-solar/30 bg-helios-solar/10 px-4 py-3">
                    <p className="text-xs font-semibold text-helios-main">Copy this token now</p>
                    <p className="mt-2 break-all font-mono text-sm text-helios-ink">{createdTokenValue}</p>
                </div>
            )}

            <form onSubmit={handleCreate} className="grid gap-4 rounded-xl border border-helios-line/20 bg-helios-surface p-4 md:grid-cols-[1fr_220px_auto]">
                <div>
                    <label className="block text-xs font-medium text-helios-slate mb-1">Token Name</label>
                    <input
                        value={name}
                        onChange={(event) => setName(event.target.value)}
                        className="w-full bg-helios-surface-soft border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                        placeholder="Home Assistant"
                        required
                    />
                </div>
                <div>
                    <label className="block text-xs font-medium text-helios-slate mb-1">Access Level</label>
                    <select
                        value={accessLevel}
                        onChange={(event) => setAccessLevel(event.target.value as ApiTokenAccessLevel)}
                        className="w-full bg-helios-surface-soft border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                    >
                        <option value="read_only">Read Only</option>
                        <option value="arr_webhook">ARR Webhook Only</option>
                        <option value="full_access">Full Access</option>
                    </select>
                </div>
                <button
                    type="submit"
                    className="self-end flex items-center justify-center gap-2 rounded-lg bg-helios-solar px-4 py-2 text-sm font-bold text-helios-main"
                >
                    <Plus size={16} />
                    Create Token
                </button>
            </form>

            {loading ? (
                <div className="text-sm text-helios-slate animate-pulse">Loading API tokens…</div>
            ) : (
                <div className="space-y-3">
                    {tokens.map((token) => (
                        <div key={token.id} className="flex items-center justify-between gap-4 rounded-xl border border-helios-line/10 bg-helios-surface p-4">
                            <div className="flex items-center gap-4">
                                <div className="rounded-lg bg-helios-surface-soft p-2 text-helios-slate">
                                    <KeyRound size={18} />
                                </div>
                                <div>
                                    <h3 className="text-sm font-bold text-helios-ink">{token.name}</h3>
                                    <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-helios-slate">
                                        <span className="rounded bg-helios-surface-soft px-1.5 py-0.5">
                                            {token.access_level}
                                        </span>
                                        <span>Created {new Date(token.created_at).toLocaleString()}</span>
                                        <span>
                                            {token.last_used_at
                                                ? `Last used ${new Date(token.last_used_at).toLocaleString()}`
                                                : "Never used"}
                                        </span>
                                        {token.revoked_at && (
                                            <span className="text-status-error">
                                                Revoked {new Date(token.revoked_at).toLocaleString()}
                                            </span>
                                        )}
                                    </div>
                                </div>
                            </div>
                            <button
                                onClick={() => setPendingDeleteId(token.id)}
                                disabled={Boolean(token.revoked_at)}
                                className="rounded-lg border border-red-500/20 p-2 text-red-500 hover:bg-red-500/10 disabled:opacity-40"
                                aria-label={`Revoke API token ${token.name}`}
                            >
                                <Trash2 size={16} />
                            </button>
                        </div>
                    ))}
                    {tokens.length === 0 && (
                        <div className="rounded-xl border border-helios-line/10 bg-helios-surface p-6 text-sm text-helios-slate">
                            No API tokens created yet.
                        </div>
                    )}
                </div>
            )}

            <ConfirmDialog
                open={pendingDeleteId !== null}
                title="Revoke API token"
                description="Revoke this token? Existing automations or scripts using it will stop working immediately."
                confirmLabel="Revoke"
                tone="danger"
                onClose={() => setPendingDeleteId(null)}
                onConfirm={async () => {
                    if (pendingDeleteId === null) return;
                    await handleRevoke(pendingDeleteId);
                    setPendingDeleteId(null);
                }}
            />
        </div>
    );
}

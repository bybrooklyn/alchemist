import { useCallback, useEffect, useMemo, useState } from "react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import AdminAccountStep from "./setup/AdminAccountStep";
import LibraryStep from "./setup/LibraryStep";
import ProcessingStep from "./setup/ProcessingStep";
import ReviewStep from "./setup/ReviewStep";
import RuntimeStep from "./setup/RuntimeStep";
import SetupFrame from "./setup/SetupFrame";
import WelcomeStep from "./setup/WelcomeStep";
import {
    DEFAULT_NOTIFICATION_DRAFT,
    DEFAULT_SCHEDULE_DRAFT,
    DEFAULT_SETTINGS,
    SETUP_STEP_COUNT,
    mergeSetupSettings,
} from "./setup/constants";
import type {
    FsPreviewResponse,
    HardwareInfo,
    NotificationTargetConfig,
    ScheduleWindowConfig,
    SettingsBundleResponse,
    SetupSettings,
    SetupStatusResponse,
} from "./setup/types";

interface FieldErrors {
    username?: string;
    password?: string;
    directories?: string;
}

const isHardwarePendingError = (err: unknown) =>
    isApiError(err) &&
    err.status === 503 &&
    err.message.toLowerCase().includes("hardware state");

export default function SetupWizard() {
    const [step, setStep] = useState(0);
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [fieldErrors, setFieldErrors] = useState<FieldErrors>({});
    const [hardware, setHardware] = useState<HardwareInfo | null>(null);
    const [configMutable, setConfigMutable] = useState(true);
    const [settings, setSettings] = useState<SetupSettings>(DEFAULT_SETTINGS);
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");
    const [dirInput, setDirInput] = useState("");
    const [scheduleDraft, setScheduleDraft] = useState<ScheduleWindowConfig>(DEFAULT_SCHEDULE_DRAFT);
    const [notificationDraft, setNotificationDraft] = useState<NotificationTargetConfig>(DEFAULT_NOTIFICATION_DRAFT);
    const [preview, setPreview] = useState<FsPreviewResponse | null>(null);
    const [previewError, setPreviewError] = useState<string | null>(null);
    const [previewLoading, setPreviewLoading] = useState(false);

    const showError = useCallback((message: string) => {
        const normalized = message.toLowerCase();
        let nextMessage = message;

        if (normalized.includes("directory") || normalized.includes("folder")) {
            nextMessage += " Go back to the Library step and verify the folder path is correct and accessible from the server.";
        }
        if (normalized.includes("concurrent")) {
            nextMessage += " Go back to the Processing step and set at least 1 concurrent job.";
        }

        setError(nextMessage);
    }, []);

    useEffect(() => {
        const loadBootstrap = async () => {
            try {
                const hardwareRequest = apiJson<HardwareInfo>("/api/system/hardware").catch((err) => {
                    if (isHardwarePendingError(err)) {
                        return null;
                    }
                    throw err;
                });
                const [status, bundle, hw] = await Promise.all([
                    apiJson<SetupStatusResponse>("/api/setup/status"),
                    apiJson<SettingsBundleResponse>("/api/settings/bundle"),
                    hardwareRequest,
                ]);
                setConfigMutable(status.config_mutable ?? true);
                setHardware(hw);
                setSettings(mergeSetupSettings(status, bundle));
                setError(null);
            } catch (err) {
                const message = isApiError(err) ? err.message : "Failed to load setup defaults.";
                showError(message);
            }
        };

        void loadBootstrap();
    }, [showError]);

    useEffect(() => {
        if (hardware !== null) {
            return;
        }

        let cancelled = false;
        let timeoutId: number | null = null;

        const pollHardware = async () => {
            try {
                const hw = await apiJson<HardwareInfo>("/api/system/hardware");
                if (!cancelled) {
                    setHardware(hw);
                }
            } catch (err) {
                if (cancelled) {
                    return;
                }
                if (isHardwarePendingError(err)) {
                    timeoutId = window.setTimeout(() => {
                        void pollHardware();
                    }, 500);
                    return;
                }
                showError(isApiError(err) ? err.message : "Failed to refresh hardware state.");
            }
        };

        timeoutId = window.setTimeout(() => {
            void pollHardware();
        }, 500);

        return () => {
            cancelled = true;
            if (timeoutId !== null) {
                window.clearTimeout(timeoutId);
            }
        };
    }, [hardware, showError]);

    const clearError = useCallback(() => {
        setError(null);
        setFieldErrors({});
    }, []);

    const handleUsernameChange = useCallback((value: string) => {
        setUsername(value);
        setFieldErrors((prev) => (prev.username ? { ...prev, username: undefined } : prev));
    }, []);

    const handlePasswordChange = useCallback((value: string) => {
        setPassword(value);
        setFieldErrors((prev) => (prev.password ? { ...prev, password: undefined } : prev));
    }, []);

    const handleDirectoriesChange = useCallback((directories: string[]) => {
        setSettings((current) => ({ ...current, scanner: { ...current.scanner, directories } }));
        if (directories.length === 0) {
            setPreview(null);
            setPreviewError(null);
            setPreviewLoading(false);
        } else {
            setPreview(null);
            setPreviewError(null);
            setPreviewLoading(true);
        }
        if (directories.length > 0) {
            setFieldErrors((prev) => (prev.directories ? { ...prev, directories: undefined } : prev));
        }
    }, []);

    // Pure, parent-owned step validation. Returns the first failing field, if any.
    const validateStep = useCallback(
        (target: number): { field: keyof FieldErrors; message: string } | null => {
            if (target === 1) {
                if (username.trim().length < 3) {
                    return { field: "username", message: "Enter an admin username (at least 3 characters)." };
                }
                if (password.trim().length < 8) {
                    return { field: "password", message: "Enter an admin password (at least 8 characters)." };
                }
            }
            if (target === 2 && settings.scanner.directories.length === 0) {
                return { field: "directories", message: "Select at least one server folder before continuing." };
            }
            if (target === 2 && previewLoading) {
                return {
                    field: "directories",
                    message: "Waiting for Alchemist to preview the selected server folders. Stay on the Library step until the preview completes.",
                };
            }
            if (target === 2 && previewError) {
                return { field: "directories", message: previewError };
            }
            if (target === 2 && settings.scanner.directories.length > 0 && preview === null) {
                return {
                    field: "directories",
                    message: "A successful library preview is required before continuing.",
                };
            }
            return null;
        },
        [username, password, preview, previewError, previewLoading, settings.scanner.directories.length]
    );

    const handleSubmit = async () => {
        setSubmitting(true);
        setError(null);
        try {
            await apiAction("/api/setup/complete", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ username, password, settings }),
            });
            // Dashboard reads this preference; write it before navigating away so the
            // request is not cancelled mid-flight by the redirect.
            await apiAction("/api/settings/preferences", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ key: "setup_complete", value: "true" }),
            }).catch((e) => { console.debug("SetupWizard: submit cleanup failed", e); });
            window.location.href = "/";
        } catch (err) {
            let message = "Failed to save setup configuration.";
            if (isApiError(err)) {
                if (err.status === 400) {
                    message = err.message.length > 0
                        ? err.message
                        : "Setup configuration was rejected. Check that your username is at least 3 characters and password is at least 8 characters.";
                } else if (err.status === 403) {
                    message = "Setup has already been completed. Redirecting to dashboard...";
                    setTimeout(() => { window.location.href = "/"; }, 1500);
                } else if (err.status >= 500) {
                    message = `Server error during setup (${err.status}). Check the Alchemist logs for details.`;
                } else {
                    message = err.message;
                }
            }
            showError(message);
        } finally {
            setSubmitting(false);
        }
    };

    const handleNext = async () => {
        const failure = validateStep(step);
        if (failure) {
            setFieldErrors({ [failure.field]: failure.message });
            setError(failure.message);
            return;
        }
        clearError();
        if (step === 5) {
            await handleSubmit();
            return;
        }
        setStep((current) => Math.min(current + 1, SETUP_STEP_COUNT));
    };

    const handleBack = () => {
        clearError();
        setStep((current) => Math.max(current - 1, 1));
    };

    // Hardware detection must resolve before the user can finish; the SSE
    // hardware_state_changed listener refreshes `hardware` and unlocks this.
    const canComplete =
        step !== 5 ||
        (hardware !== null && preview !== null && !previewLoading && previewError === null);

    const previewSummaryValue = previewLoading
        ? "Previewing"
        : previewError
            ? "Preview failed"
            : preview
                ? `${preview.total_media_files}`
                : settings.scanner.directories.length > 0
                    ? "Preview required"
                    : "--";

    const setupSummary = useMemo(
        () => [
            { label: "Server folders", value: `${settings.scanner.directories.length}` },
            { label: "Previewed media files", value: previewSummaryValue },
            { label: "Notification targets", value: `${settings.notifications.targets.length}` },
            { label: "Schedule windows", value: `${settings.schedule.windows.length}` },
        ],
        [
            previewSummaryValue,
            settings.notifications.targets.length,
            settings.schedule.windows.length,
            settings.scanner.directories.length,
        ]
    );

    const currentStep = (() => {
        switch (step) {
            case 0:
                return (
                    <WelcomeStep
                        onGetStarted={() => setStep(1)}
                    />
                );
            case 1:
                return (
                    <AdminAccountStep
                        username={username}
                        password={password}
                        usernameError={fieldErrors.username}
                        passwordError={fieldErrors.password}
                        onUsernameChange={handleUsernameChange}
                        onPasswordChange={handlePasswordChange}
                        onEnter={() => void handleNext()}
                    />
                );
            case 2:
                return (
                    <LibraryStep
                        dirInput={dirInput}
                        directories={settings.scanner.directories}
                        directoriesError={fieldErrors.directories}
                        previewError={previewError}
                        onDirInputChange={setDirInput}
                        onDirectoriesChange={handleDirectoriesChange}
                        onPreviewChange={setPreview}
                        onPreviewErrorChange={setPreviewError}
                        onPreviewLoadingChange={setPreviewLoading}
                    />
                );
            case 3:
                return (
                    <ProcessingStep
                        transcode={settings.transcode}
                        files={settings.files}
                        quality={settings.quality}
                        onTranscodeChange={(transcode) => setSettings((current) => ({ ...current, transcode }))}
                        onFilesChange={(files) => setSettings((current) => ({ ...current, files }))}
                        onQualityChange={(quality) => setSettings((current) => ({ ...current, quality }))}
                    />
                );
            case 4:
                return (
                    <RuntimeStep
                        hardwareInfo={hardware}
                        hardware={settings.hardware}
                        notifications={settings.notifications}
                        schedule={settings.schedule}
                        scheduleDraft={scheduleDraft}
                        notificationDraft={notificationDraft}
                        onHardwareChange={(hardwareSettings) => setSettings((current) => ({ ...current, hardware: hardwareSettings }))}
                        onNotificationsChange={(notifications) => setSettings((current) => ({ ...current, notifications }))}
                        onScheduleChange={(schedule) => setSettings((current) => ({ ...current, schedule }))}
                        onScheduleDraftChange={setScheduleDraft}
                        onNotificationDraftChange={setNotificationDraft}
                    />
                );
            case 5:
                return (
                    <ReviewStep
                        setupSummary={setupSummary}
                        settings={settings}
                        preview={preview}
                        previewError={previewError}
                        previewLoading={previewLoading}
                        hardware={hardware}
                    />
                );
            default:
                return null;
        }
    })();

    return (
        <SetupFrame
            step={step}
            configMutable={configMutable}
            canComplete={canComplete}
            error={error}
            submitting={submitting}
            onBack={handleBack}
            onNext={() => void handleNext()}
        >
            {currentStep}
        </SetupFrame>
    );
}

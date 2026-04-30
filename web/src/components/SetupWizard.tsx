import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
    FsRecommendation,
    FsRecommendationsResponse,
    HardwareInfo,
    NotificationTargetConfig,
    ScheduleWindowConfig,
    SettingsBundleResponse,
    SetupSettings,
    SetupStatusResponse,
    StepValidator,
} from "./setup/types";

const isHardwarePendingError = (err: unknown) =>
    isApiError(err) &&
    err.status === 503 &&
    err.message.toLowerCase().includes("hardware state");

export default function SetupWizard() {
    const [step, setStep] = useState(0);
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [hardware, setHardware] = useState<HardwareInfo | null>(null);
    const [configMutable, setConfigMutable] = useState(true);
    const [settings, setSettings] = useState<SetupSettings>(DEFAULT_SETTINGS);
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");
    const [dirInput, setDirInput] = useState("");
    const [scheduleDraft, setScheduleDraft] = useState<ScheduleWindowConfig>(DEFAULT_SCHEDULE_DRAFT);
    const [notificationDraft, setNotificationDraft] = useState<NotificationTargetConfig>(DEFAULT_NOTIFICATION_DRAFT);
    const [recommendations, setRecommendations] = useState<FsRecommendation[]>([]);
    const [preview, setPreview] = useState<FsPreviewResponse | null>(null);
    const validatorRef = useRef<StepValidator>(async () => null);

    const registerValidator = useCallback((validator: StepValidator) => {
        validatorRef.current = validator;
    }, []);

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
        setTimeout(() => {
            document.querySelector('[role="alert"]')
                ?.scrollIntoView({ behavior: "smooth", block: "center" });
        }, 100);
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
                const [status, bundle, hw, recommendationData] = await Promise.all([
                    apiJson<SetupStatusResponse>("/api/setup/status"),
                    apiJson<SettingsBundleResponse>("/api/settings/bundle"),
                    hardwareRequest,
                    apiJson<FsRecommendationsResponse>("/api/fs/recommendations"),
                ]);
                setConfigMutable(status.config_mutable ?? true);
                setHardware(hw);
                setRecommendations(recommendationData.recommendations);
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
        const eventSource = new EventSource("/api/events");
        eventSource.addEventListener("hardware_state_changed", () => {
            void apiJson<HardwareInfo>("/api/system/hardware")
                .then((hw) => setHardware(hw))
                .catch((err) => {
                    if (!isHardwarePendingError(err)) {
                        showError(isApiError(err) ? err.message : "Failed to refresh hardware state.");
                    }
                });
        });

        return () => eventSource.close();
    }, [showError]);

    const handleSubmit = async () => {
        setSubmitting(true);
        setError(null);
        try {
            await apiAction("/api/setup/complete", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ username, password, settings }),
            });
            void apiAction("/api/settings/preferences", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ key: "setup_complete", value: "true" }),
            }).catch(() => undefined);
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
        const validationMessage = await validatorRef.current();
        if (validationMessage) {
            showError(validationMessage);
            return;
        }
        setError(null);
        if (step === 5) {
            await handleSubmit();
            return;
        }

        setStep((current) => Math.min(current + 1, SETUP_STEP_COUNT));
    };

    const setupSummary = useMemo(
        () => [
            { label: "Server folders", value: `${settings.scanner.directories.length}` },
            { label: "Previewed media files", value: preview ? `${preview.total_media_files}` : "--" },
            { label: "Notification targets", value: `${settings.notifications.targets.length}` },
            { label: "Schedule windows", value: `${settings.schedule.windows.length}` },
        ],
        [preview, settings.notifications.targets.length, settings.schedule.windows.length, settings.scanner.directories.length]
    );

    validatorRef.current = async () => null;

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
                        onUsernameChange={setUsername}
                        onPasswordChange={setPassword}
                        registerValidator={registerValidator}
                    />
                );
            case 2:
                return (
                    <LibraryStep
                        dirInput={dirInput}
                        directories={settings.scanner.directories}
                        recommendations={recommendations}
                        onDirInputChange={setDirInput}
                        onDirectoriesChange={(directories) => setSettings((current) => ({ ...current, scanner: { ...current.scanner, directories } }))}
                        onPreviewChange={setPreview}
                        registerValidator={registerValidator}
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
                return <ReviewStep setupSummary={setupSummary} settings={settings} preview={preview} error={null} />;
            default:
                return null;
        }
    })();

    return (
        <SetupFrame
            step={step}
            configMutable={configMutable}
            error={error}
            submitting={submitting}
            onBack={() => setStep((current) => Math.max(current - 1, 1))}
            onNext={() => void handleNext()}
        >
            {currentStep}
        </SetupFrame>
    );
}

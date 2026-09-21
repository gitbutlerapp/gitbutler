/** @vitest-environment jsdom */

import { Toast } from "@base-ui/react";
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, StrictMode, use, type FC } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { InstallationStatus } from "#electron/updater-state.ts";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { AppUpdater } from "./AppUpdater.tsx";
import { CheckForUpdatesContext } from "./updater-context.ts";
import { Toasts } from "@gitbutler/ui-react/Toasts.tsx";
import { reportError } from "#ui/error-reporting.ts";

const getUpdateStatus = vi.hoisted(() => {
	const getUpdateStatus = vi.fn();
	vi.stubGlobal("lite", { getUpdateStatus });
	return getUpdateStatus;
});

vi.mock("#ui/error-reporting.ts", () => ({ reportError: vi.fn() }));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

let client: QueryClient;
let root: Root;
let state: InstallationStatus;
let publish: (state: InstallationStatus) => void;
let manual: () => void;
const stateListeners = new Set<(state: InstallationStatus) => void>();
const checkForUpdates = vi.fn();
const downloadUpdate = vi.fn();
const installUpdate = vi.fn();
const checkButtonRendered = vi.fn();

beforeEach(() => {
	vi.resetAllMocks();
	getUpdateStatus.mockImplementation(async () => state);
	vi.useFakeTimers();
	state = { _tag: "Idle" };
	stateListeners.clear();
	publish = (next) => {
		state = next;
		for (const listener of stateListeners) listener(next);
	};
	manual = () => document.querySelector<HTMLButtonElement>('[aria-label="Check now"]')?.click();
	vi.stubGlobal("lite", {
		getUpdateStatus,
		checkForUpdates,
		downloadUpdate,
		installUpdate,
		onUpdateStatusChange: (listener: typeof publish) => {
			stateListeners.add(listener);
			return () => stateListeners.delete(listener);
		},
	});
	checkForUpdates.mockResolvedValue({ _tag: "Available", version: "0.0.201" });
	downloadUpdate.mockImplementation(async (version: string) => {
		state = { _tag: "Downloading", version };
		publish(state);
	});
	installUpdate.mockResolvedValue(undefined);
	client = new QueryClient({
		defaultOptions: { queries: { retry: false, staleTime: Infinity } },
		queryCache: new QueryCache({ onError: (error) => reportError(error) }),
		mutationCache: new MutationCache({ onError: (error) => reportError(error) }),
	});
	const container = document.createElement("div");
	document.body.append(container);
	root = createRoot(container);
});

afterEach(() => {
	act(() => root.unmount());
	client.clear();
	document.body.replaceChildren();
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

const settle = async (ms = 1) => {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(ms);
	});
};

const CheckButton: FC = () => {
	const checkForUpdates = use(CheckForUpdatesContext);
	checkButtonRendered(checkForUpdates);
	return <button type="button" aria-label="Check now" onClick={checkForUpdates} />;
};

const mount = async (autoUpdate: boolean) => {
	client.setQueryData(guiSettingsQueryOptions.queryKey, { version: 1, autoUpdate });
	act(() =>
		root.render(
			<StrictMode>
				<QueryClientProvider client={client}>
					<Toast.Provider>
						<AppUpdater>
							<CheckButton />
						</AppUpdater>
						<Toasts />
					</Toast.Provider>
				</QueryClientProvider>
			</StrictMode>,
		),
	);
	await settle();
	await settle();
};

const click = async (label: string) => {
	const button = [...document.querySelectorAll("button")].find(
		(item) => item.textContent === label,
	);
	expect(button, label).toBeDefined();
	act(() => button?.click());
	await settle();
};

it("owns the status subscription through StrictMode replay and stops polling on unmount", async () => {
	expect(stateListeners.size).toBe(0);
	await mount(true);
	expect(stateListeners.size).toBe(1);
	act(() => root.render(null));
	expect(stateListeners.size).toBe(0);
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).toHaveBeenCalledOnce();
	publish({ _tag: "Ready", version: "0.0.201" });
	await mount(true);
	expect(stateListeners.size).toBe(1);
	expect(document.body.textContent).toContain("Install 0.0.201 now");
});

it("polls hourly once, respects the setting, and permits a manual check", async () => {
	await mount(false);
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).not.toHaveBeenCalled();
	act(() => manual());
	await settle();
	expect(checkForUpdates).toHaveBeenCalledOnce();
	act(() => {
		client.setQueryData(guiSettingsQueryOptions.queryKey, { version: 1, autoUpdate: true });
	});
	await settle();
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).toHaveBeenCalledTimes(2);
	act(() => {
		client.setQueryData(guiSettingsQueryOptions.queryKey, { version: 1, autoUpdate: false });
	});
	await settle();
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).toHaveBeenCalledTimes(2);
});

it("checks once on startup, keeps dismissed versions quiet, and shows a newer version", async () => {
	await mount(true);
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(document.body.textContent).toContain("Download 0.0.201");
	await click("Dismiss");
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).toHaveBeenCalledTimes(2);
	expect(document.body.textContent).not.toContain("Download 0.0.201");
	checkForUpdates.mockResolvedValueOnce({ _tag: "Available", version: "0.0.202" });
	await settle(60 * 60 * 1000);
	expect(document.body.textContent).toContain("Download 0.0.202");
});

it("downloads only after consent and follows progress through restart", async () => {
	await mount(true);
	expect(document.querySelector("strong")?.textContent).toBe("Check for updates");
	expect(document.body.textContent).toContain(
		"Update available. Download the update now and restart when you're ready.",
	);
	expect(downloadUpdate).not.toHaveBeenCalled();
	await click("Download 0.0.201");
	expect(downloadUpdate).toHaveBeenCalledOnce();
	expect(downloadUpdate.mock.calls[0]?.[0]).toBe("0.0.201");
	expect(document.body.textContent).toContain("Preparing update 0.0.201");
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).toHaveBeenCalledTimes(2);
	act(() => publish({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	expect(document.body.textContent).toContain("Update downloaded. Restart now or install on quit.");
	await click("Install 0.0.201 now");
	expect(installUpdate).toHaveBeenCalledOnce();
});

it("recovers an existing download when the renderer mounts", async () => {
	state = { _tag: "Ready", version: "0.0.201" };
	publish(state);
	expect(document.body.textContent).toBe("");
	await mount(true);
	expect(document.body.textContent).toContain("Install 0.0.201 now");
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(client.getQueryData(["updateStatus"])).toEqual(state);
});

it("records a rejected mutation and reports an event-plus-rejection failure once", async () => {
	const { promise, reject } = Promise.withResolvers<void>();
	downloadUpdate.mockImplementationOnce((version: string) => {
		publish({ _tag: "Downloading", version });
		return promise;
	});
	await mount(true);
	await click("Download 0.0.201");
	expect(document.body.textContent).toContain("Preparing update 0.0.201");
	act(() => {
		publish({ _tag: "Idle" });
		reject(new Error("Download failed"));
	});
	await settle();
	expect(client.getMutationCache().find({ mutationKey: ["downloadAppUpdate"] })?.state.status).toBe(
		"error",
	);
	expect(reportError).toHaveBeenCalledOnce();
	expect(document.body.textContent.match(/Failed to download app update/g)).toHaveLength(1);
	expect(document.body.textContent).toContain("Failed to download app update: Download failed");
	expect(document.body.textContent).not.toContain("Download 0.0.201");
	expect(document.body.textContent).not.toContain("Preparing update");
	expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
	act(() => manual());
	await settle();
	expect(document.body.textContent).toContain("Download 0.0.201");
	expect(document.body.textContent).not.toContain("Failed to download app update");
	await click("Download 0.0.201");
	expect(document.body.textContent).toContain("Preparing update 0.0.201");
	expect(document.body.textContent).not.toContain("Failed to download app update");
});

it("rejects a pending install through the mutation cache without reporting its event twice", async () => {
	state = { _tag: "Ready", version: "0.0.201" };
	let rejectInstall!: (error: Error) => void;
	installUpdate.mockImplementationOnce(
		() =>
			new Promise<void>((_resolve, reject) => {
				publish({ _tag: "Installing", version: "0.0.201" });
				rejectInstall = reject;
			}),
	);
	await mount(false);
	await click("Install 0.0.201 now");
	expect(client.getMutationCache().find({ mutationKey: ["installAppUpdate"] })?.state.status).toBe(
		"pending",
	);
	expect(document.body.textContent).toContain("Restarting...");
	const error = new Error("Installer failed");
	act(() => {
		publish({ _tag: "Idle" });
		rejectInstall(error);
	});
	await settle();
	expect(client.getMutationCache().find({ mutationKey: ["installAppUpdate"] })?.state.status).toBe(
		"error",
	);
	expect(reportError).toHaveBeenCalledExactlyOnceWith(error);
	expect(document.body.textContent).toContain(`Failed to install app update: ${error.message}`);
	expect(document.body.textContent).not.toContain("Restarting...");
	expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
});

it("stops polling unavailable builds while allowing another manual check", async () => {
	checkForUpdates.mockResolvedValue({ _tag: "Unavailable" });
	await mount(true);
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(document.body.textContent).toBe("");
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).toHaveBeenCalledOnce();
	act(() => manual());
	await settle();
	expect(checkForUpdates).toHaveBeenCalledTimes(2);
	expect(document.body.textContent).toContain("Updates are unavailable in this build.");
});

it("keeps automatic check failures quiet but shows manual failures", async () => {
	checkForUpdates.mockRejectedValue(new Error("Offline"));
	await mount(true);
	expect(document.body.textContent).toBe("");
	expect(client.getQueryState(["updateCheck"])?.status).toBe("error");
	expect(client.getQueryData(["updateStatus"])).toEqual({ _tag: "Idle" });
	expect(reportError).toHaveBeenCalledOnce();
	act(() => manual());
	await settle();
	expect(document.body.textContent).toContain("Failed to check for updates: Offline");
});

it.each([
	["Unavailable", "Updates are unavailable in this build."],
	["UpToDate", "You are up to date."],
] as const)("keeps the first manual %s result visible", async (_tag, message) => {
	checkForUpdates.mockResolvedValue({ _tag });
	await mount(false);
	act(() => manual());
	await settle();
	expect(document.body.textContent).toContain(message);
	expect(document.body.textContent).not.toContain("Checking for updates...");
	expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
});

it.each([
	["Unavailable", "Updates are unavailable in this build."],
	["UpToDate", "You are up to date."],
] as const)("shows an unchanged %s result after a manual check", async (_tag, message) => {
	checkForUpdates.mockResolvedValue({ _tag });
	await mount(true);
	act(() => manual());
	await settle();
	expect(document.body.textContent).toContain(message);
});

it("replaces the previous update offer with the manual check result", async () => {
	await mount(true);
	expect(document.body.textContent).toContain("Download 0.0.201");
	checkForUpdates.mockResolvedValueOnce({ _tag: "UpToDate" });
	act(() => manual());
	await settle();
	expect(document.body.textContent).toContain("You are up to date.");
	expect(document.body.textContent).not.toContain("Download 0.0.201");
});

it("leaves an existing offer alone when a background check finds no update", async () => {
	await mount(true);
	checkForUpdates.mockResolvedValueOnce({ _tag: "UpToDate" });
	await settle(60 * 60 * 1000);
	expect(document.body.textContent).toContain("Download 0.0.201");
	expect(document.body.textContent).not.toContain("You are up to date.");
});

it("reopens a dismissed offer when a manual check returns the same version", async () => {
	await mount(true);
	await click("Dismiss");
	act(() => manual());
	await settle();
	expect(checkForUpdates).toHaveBeenCalledTimes(2);
	expect(document.body.textContent).toContain("Download 0.0.201");
	expect(document.body.textContent).not.toContain("Checking for updates...");
	await click("Dismiss");
	act(() => manual());
	await settle();
	expect(checkForUpdates).toHaveBeenCalledTimes(3);
	expect(document.body.textContent).toContain("Download 0.0.201");
	await click("Dismiss");
	await settle(60 * 60 * 1000);
	expect(document.body.textContent).not.toContain("Download 0.0.201");
});

it("joins an ongoing check and presents the latest installation status when it completes", async () => {
	state = { _tag: "Ready", version: "0.0.201" };
	let finishCheck!: (result: { _tag: "Available"; version: string }) => void;
	checkForUpdates.mockImplementationOnce(
		() =>
			new Promise((resolve) => {
				finishCheck = resolve;
			}),
	);
	await mount(true);
	act(() => manual());
	await settle();
	expect(document.body.textContent).toContain("Checking for updates...");
	expect(document.body.textContent).not.toContain("Install 0.0.201 now");
	expect(document.body.textContent.match(/Dismiss/g)).toHaveLength(1);
	act(() => publish({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	act(() => finishCheck({ _tag: "Available", version: "0.0.201" }));
	await settle();
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(document.body.textContent).toContain("Install 0.0.201 now");
	expect(document.body.textContent).not.toContain("Checking for updates...");
	expect(document.body.textContent).not.toContain("Download 0.0.201");
});

it("uses the latest status when a manual check returns an unchanged result", async () => {
	state = { _tag: "Ready", version: "0.0.201" };
	await mount(true);
	const { promise, resolve } = Promise.withResolvers<{ _tag: "Available"; version: string }>();
	checkForUpdates.mockReturnValueOnce(promise);
	act(() => manual());
	await settle();
	act(() => publish({ _tag: "Installing", version: "0.0.201" }));
	await settle();
	act(() => resolve({ _tag: "Available", version: "0.0.201" }));
	await settle();
	expect(document.body.textContent).toContain("Restarting...");
	expect(document.body.textContent).not.toContain("Install 0.0.201 now");
});

it("keeps the context consumer from rerendering as updater and toast state change", async () => {
	await mount(true);
	const renders = checkButtonRendered.mock.calls.length;
	expect(renders).toBeGreaterThan(0);
	await click("Dismiss");
	act(() => manual());
	await settle();
	act(() => publish({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	checkForUpdates.mockResolvedValue({ _tag: "Available", version: "0.0.202" });
	await settle(60 * 60 * 1000);
	expect(document.body.textContent).toContain("Download 0.0.202");
	expect(checkButtonRendered).toHaveBeenCalledTimes(renders);
});

it("does not replay a manual check error when installation status changes", async () => {
	await mount(false);
	const error = new Error("Offline");
	checkForUpdates.mockRejectedValueOnce(error);
	act(() => manual());
	await settle();
	expect(document.body.textContent).toContain("Failed to check for updates: Offline");
	expect(reportError).toHaveBeenCalledExactlyOnceWith(error);
	await click("Dismiss");
	act(() => publish({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	expect(document.body.textContent).toContain("Install 0.0.201 now");
	expect(document.body.textContent).not.toContain("Failed to check for updates");
});

it("clears the download action and offer description when progress replaces the offer", async () => {
	await mount(true);
	await click("Download 0.0.201");
	expect(document.body.textContent).toContain("Preparing update 0.0.201");
	expect(document.body.textContent).not.toContain("Download 0.0.201");
	expect(document.body.textContent).not.toContain("Download the update now");
});

it("updates download measurements in one toast and waits for preparation before offering install", async () => {
	await mount(true);
	await click("Download 0.0.201");
	act(() =>
		publish({
			_tag: "Downloading",
			version: "0.0.201",
			progress: {
				percent: 42,
				transferred: 63_000_000,
				total: 150_000_000,
				delta: 1_000_000,
				bytesPerSecond: 500_000,
			},
		}),
	);
	await settle();
	expect(document.body.textContent).toContain("42%");
	expect(document.body.textContent).toContain("63 MB / 150 MB");
	expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
	act(() =>
		publish({
			_tag: "Downloading",
			version: "0.0.201",
			progress: {
				percent: 100,
				transferred: 150_000_000,
				total: 150_000_000,
				delta: 87_000_000,
				bytesPerSecond: 500_000,
			},
		}),
	);
	await settle();
	expect(document.body.textContent).toContain("Preparing update 0.0.201...");
	expect(document.body.textContent).not.toContain("Install 0.0.201 now");
	act(() => publish({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	expect(document.body.textContent).toContain("Install 0.0.201 now");
	expect(document.body.textContent).not.toContain("Preparing update");
});

it("does not reopen a dismissed download toast on progress ticks", async () => {
	await mount(true);
	await click("Download 0.0.201");
	await click("Dismiss");
	act(() =>
		publish({
			_tag: "Downloading",
			version: "0.0.201",
			progress: {
				percent: 42,
				transferred: 63_000_000,
				total: 150_000_000,
				delta: 1_000_000,
				bytesPerSecond: 500_000,
			},
		}),
	);
	await settle();
	expect(document.body.textContent).toBe("");
	act(() => publish({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	expect(document.body.textContent).toContain("Install 0.0.201 now");
});

it("recovers download measurements when the renderer mounts", async () => {
	state = {
		_tag: "Downloading",
		version: "0.0.201",
		progress: {
			percent: 42,
			transferred: 63_000_000,
			total: 150_000_000,
			delta: 1_000_000,
			bytesPerSecond: 500_000,
		},
	};
	await mount(false);
	expect(document.body.textContent).toContain("42%");
	expect(document.body.textContent).toContain("63 MB / 150 MB");
});

it("does not trigger additional checks on rerenders or repeated status events", async () => {
	await mount(true);
	await mount(true);
	act(() => {
		publish({ _tag: "Idle" });
		publish({ _tag: "Idle" });
	});
	await settle();
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(getUpdateStatus).toHaveBeenCalledOnce();
	expect(stateListeners.size).toBe(1);
	expect(document.body.textContent.match(/Update available/g)).toHaveLength(1);
	await settle(60 * 60 * 1000);
	expect(checkForUpdates).toHaveBeenCalledTimes(2);
	expect(document.body.textContent.match(/Update available/g)).toHaveLength(1);
});

it("checks while the native snapshot is pending without seeding the status cache", async () => {
	let resolveStatus!: (status: InstallationStatus) => void;
	getUpdateStatus.mockImplementationOnce(
		() =>
			new Promise<InstallationStatus>((resolve) => {
				resolveStatus = resolve;
			}),
	);
	await mount(true);
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(client.getQueryData(["updateStatus"])).toBeUndefined();
	expect(document.body.textContent).toContain("Download 0.0.201");
	act(() => resolveStatus({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(document.body.textContent).toContain("Install 0.0.201 now");
});

it("reports an initial snapshot failure through the query cache and keeps the updater mounted", async () => {
	const error = new Error("IPC unavailable");
	getUpdateStatus.mockRejectedValueOnce(error);
	await mount(true);
	expect(client.getQueryState(["updateStatus"])?.status).toBe("error");
	expect(client.getQueryData(["updateStatus"])).toBeUndefined();
	expect(reportError).toHaveBeenCalledExactlyOnceWith(error);
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(stateListeners.size).toBe(1);
	expect(document.body.textContent).toContain("Download 0.0.201");
});

it("uses one toast when a manual check discovers an update for the first time", async () => {
	await mount(false);
	act(() => manual());
	await settle();
	expect(document.body.textContent.match(/Download 0.0.201/g)).toHaveLength(1);
	expect(document.body.textContent).not.toContain("Checking for updates...");
	await click("Download 0.0.201");
	expect(document.body.textContent.match(/Preparing update 0.0.201/g)).toHaveLength(1);
	expect(document.body.textContent).not.toContain("Download 0.0.201");
});

it.each([
	["Downloading", "Preparing update 0.0.201"],
	["Ready", "Install 0.0.201 now"],
	["Installing", "Restarting..."],
] as const)("preserves %s after a real check finds the same release", async (tag, text) => {
	state = { _tag: tag, version: "0.0.201" };
	await mount(false);
	await click("Dismiss");
	act(() => manual());
	await settle();
	expect(checkForUpdates).toHaveBeenCalledOnce();
	expect(document.body.textContent).toContain(text);
	expect(document.body.textContent).not.toContain("Download 0.0.201");
});

it("replaces the restart action with progress when installation starts", async () => {
	state = { _tag: "Ready", version: "0.0.201" };
	await mount(false);
	act(() => publish({ _tag: "Installing", version: "0.0.201" }));
	await settle();
	expect(document.body.textContent).toContain("Restarting...");
	expect(document.body.textContent).not.toContain("Install 0.0.201 now");
});

it("gives a newer release precedence while the older download finishes", async () => {
	await mount(true);
	await click("Download 0.0.201");
	checkForUpdates.mockResolvedValue({ _tag: "Available", version: "0.0.202" });
	act(() => manual());
	await settle();
	const offer = () =>
		[...document.querySelectorAll("button")].find(
			(button) => button.textContent === "Download 0.0.202",
		);
	expect(offer()?.disabled).toBe(true);
	act(() => publish({ _tag: "Ready", version: "0.0.201" }));
	await settle();
	expect(offer()?.disabled).toBe(false);
	expect(document.body.textContent).not.toContain("Install 0.0.201 now");
	await click("Download 0.0.202");
	expect(downloadUpdate).toHaveBeenCalledTimes(2);
	expect(downloadUpdate.mock.calls[1]?.[0]).toBe("0.0.202");
});

it("shows an immediate installation failure after native status returns to idle", async () => {
	state = { _tag: "Ready", version: "0.0.201" };
	installUpdate.mockImplementationOnce(async () => {
		publish({ _tag: "Installing", version: "0.0.201" });
		await Promise.resolve();
		publish({ _tag: "Idle" });
		throw new Error("Installer failed");
	});
	await mount(false);
	await click("Install 0.0.201 now");
	expect(document.body.textContent).toContain("Failed to install app update: Installer failed");
	expect(document.body.textContent).not.toContain("Restarting...");
	expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
});

import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Channel, InvokeArgs } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DownloadEvent } from "@tauri-apps/plugin-updater";

vi.stubGlobal("window", globalThis);

const host = {
	version: "0.0.200",
	checkIntervalSeconds: 3600,
	commands: [] as Array<string>,
};

const downloadEvents: Array<DownloadEvent> = [
	{ event: "Started", data: { contentLength: 1000 } },
	{ event: "Progress", data: { chunkLength: 4 } },
	{ event: "Progress", data: { chunkLength: 496 } },
	{ event: "Progress", data: { chunkLength: 1 } },
	{ event: "Progress", data: { chunkLength: 499 } },
	{ event: "Finished" },
];

const ipc = (command: string, args?: InvokeArgs): unknown => {
	host.commands.push(command);
	switch (command) {
		case "plugin:app|version":
			return host.version;
		case "get_app_settings":
			return { ui: { checkForUpdatesIntervalInSeconds: host.checkIntervalSeconds } };
		case "plugin:updater|check":
			return { rid: 1, currentVersion: host.version, version: "0.0.201", rawJson: {} };
		case "plugin:updater|download": {
			const { onEvent } = args as { onEvent: Channel<DownloadEvent> };
			for (const event of downloadEvents) onEvent.onmessage(event);
			return 2;
		}
		case "plugin:updater|install":
		case "plugin:process|restart":
			return null;
	}
	throw new Error(`Unexpected command ${command}`);
};

const loadUpdater = async () => {
	vi.resetModules();
	return await import("../../tauri/updater.ts");
};

beforeEach(() => {
	host.version = "0.0.200";
	host.checkIntervalSeconds = 3600;
	host.commands = [];
	mockIPC(ipc);
});

afterEach(() => clearMocks());

it("checks, downloads with progress, then installs and relaunches", async () => {
	const { handlers, channels } = await loadUpdater();
	const statuses: Array<unknown> = [];
	channels.updateStatusChange?.((status) => statuses.push(status));

	expect(await handlers.checkForUpdates()).toEqual({ _tag: "Available", version: "0.0.201" });
	await handlers.downloadUpdate("0.0.201");
	await handlers.installUpdate();

	const progress = (transferred: number, delta: number) => ({
		total: 1000,
		transferred,
		percent: transferred / 10,
		delta,
	});
	expect(statuses).toMatchObject([
		{ _tag: "Downloading", version: "0.0.201" },
		{ _tag: "Downloading", version: "0.0.201", progress: progress(500, 500) },
		{ _tag: "Downloading", version: "0.0.201", progress: progress(1000, 500) },
		{ _tag: "Ready", version: "0.0.201" },
		{ _tag: "Installing", version: "0.0.201" },
	]);
	expect(handlers.getUpdateStatus()).toEqual({ _tag: "Installing", version: "0.0.201" });
	expect(host.commands.slice(-2)).toEqual(["plugin:updater|install", "plugin:process|restart"]);
});

it("refuses to download a version the last check didn't offer", async () => {
	const { handlers } = await loadUpdater();
	await handlers.checkForUpdates();

	await expect(handlers.downloadUpdate("0.0.202")).rejects.toThrow("The available update changed");
	expect(handlers.getUpdateStatus()).toEqual({ _tag: "Idle" });
});

it.each([
	["local builds", { version: "0.0.0" }],
	["builds with auto updates disabled", { checkIntervalSeconds: 0 }],
])("is unavailable for %s", async (_, overrides) => {
	Object.assign(host, overrides);
	const { handlers } = await loadUpdater();

	expect(await handlers.checkForUpdates()).toEqual({ _tag: "Unavailable" });
	expect(host.commands).not.toContain("plugin:updater|check");
});

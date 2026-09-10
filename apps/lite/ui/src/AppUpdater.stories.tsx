import preview from "#storybook/preview";
import { Field, Toast } from "@base-ui/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { FC } from "react";
import { valid } from "semver";
import type { LiteElectronApi } from "#electron/ipc.ts";
import type { InstallationStatus } from "#electron/updater-state.ts";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { AppUpdater } from "#ui/AppUpdater.tsx";
import { CheckForUpdatesButton } from "#ui/CheckForUpdatesButton.tsx";
import { FieldControlStyles, FieldLabelStyles, FieldRootStyles } from "#ui/components/Field.tsx";
import { Switch } from "#ui/components/Switch.tsx";
import { Toasts } from "#ui/components/Toasts.tsx";

type Options = {
	result: "Available" | "UpToDate" | "Unavailable" | "Error";
	version: string;
	delayMs: number;
	failure: "None" | "Download" | "Install";
};

let demo: { client: QueryClient; options: Options };

const Demo: FC = () => {
	const { client, options } = demo;
	return (
		<QueryClientProvider client={client}>
			<Toast.Provider>
				<AppUpdater>
					<div style={{ display: "grid", gap: 16, maxWidth: 360 }}>
						<p className="text-13">
							Simulated updates. Installation stops at “Restarting…”. Use “Reload story” to start
							over.
						</p>
						<Field.Root render={<FieldRootStyles />}>
							<Field.Label render={<FieldLabelStyles />}>Check result</Field.Label>
							<Field.Control
								render={<select aria-label="Check result" />}
								defaultValue={options.result}
								onChange={(e) => {
									options.result = e.currentTarget.value as Options["result"];
								}}
							>
								<option value="Available">Update available</option>
								<option value="UpToDate">Up to date</option>
								<option value="Unavailable">Unavailable</option>
								<option value="Error">Check fails</option>
							</Field.Control>
						</Field.Root>
						<Field.Root render={<FieldRootStyles />}>
							<Field.Label render={<FieldLabelStyles />}>Available version</Field.Label>
							<Field.Control
								render={<FieldControlStyles />}
								defaultValue={options.version}
								onChange={(e) => {
									options.version = e.currentTarget.value;
								}}
							/>
						</Field.Root>
						<Field.Root render={<FieldRootStyles />}>
							<Field.Label render={<FieldLabelStyles />}>Delay per step (ms)</Field.Label>
							<Field.Control
								render={<FieldControlStyles />}
								type="number"
								min={0}
								step={100}
								defaultValue={options.delayMs}
								onChange={(e) => {
									options.delayMs = Math.max(0, Number(e.currentTarget.value));
								}}
							/>
						</Field.Root>
						<Field.Root render={<FieldRootStyles />}>
							<Field.Label render={<FieldLabelStyles />}>Failure stage</Field.Label>
							<Field.Control
								render={<select aria-label="Failure stage" />}
								defaultValue={options.failure}
								onChange={(e) => {
									options.failure = e.currentTarget.value as Options["failure"];
								}}
							>
								<option>None</option>
								<option>Download</option>
								<option>Install</option>
							</Field.Control>
						</Field.Root>
						<div style={{ display: "flex", alignItems: "center", gap: 12 }}>
							<CheckForUpdatesButton />
							<Switch
								aria-label="Check for updates automatically"
								defaultChecked={false}
								onCheckedChange={(autoUpdate) =>
									client.setQueryData(guiSettingsQueryOptions.queryKey, {
										version: 1,
										autoUpdate,
									})
								}
							/>
							<span className="text-12">Automatic checks</span>
						</div>
					</div>
				</AppUpdater>
				<Toasts />
			</Toast.Provider>
		</QueryClientProvider>
	);
};

const meta = preview.meta({
	title: "App/Updater",
	component: Demo,
	beforeEach: () => {
		let status: InstallationStatus = { _tag: "Idle" };
		const client = new QueryClient({
			defaultOptions: { queries: { retry: false, staleTime: Infinity } },
		});
		const options: Options = {
			result: "Available",
			version: "0.0.201",
			delayMs: 1500,
			failure: "None",
		};
		const listeners = new Set<(status: InstallationStatus) => void>();
		const timers = new Set<ReturnType<typeof setTimeout>>();
		const delay = () =>
			new Promise<void>((resolve) => {
				const timer = setTimeout(() => {
					timers.delete(timer);
					resolve();
				}, options.delayMs);
				timers.add(timer);
			});
		const publish = (next: InstallationStatus) => {
			status = next;
			for (const listener of listeners) listener(next);
		};
		const api = {
			getUpdateStatus: async () => status,
			checkForUpdates: async () => {
				const { result, version } = options;
				await delay();
				if (result === "Error") throw new Error("Simulated check failure");
				if (result !== "Available") return { _tag: result };
				if (valid(version) === null) throw new Error("Enter a valid release version");
				return { _tag: "Available", version };
			},
			downloadUpdate: async (version) => {
				const { failure } = options;
				let transferred = 0;
				publish({ _tag: "Downloading", version });
				for (const percent of [10, 30, 60, 85, 100]) {
					await delay();
					if (failure === "Download" && percent === 60) {
						publish({ _tag: "Idle" });
						throw new Error("Simulated download failure");
					}
					const delta = (150_000_000 * percent) / 100 - transferred;
					transferred += delta;
					publish({
						_tag: "Downloading",
						version,
						progress: {
							percent,
							transferred,
							total: 150_000_000,
							delta,
							bytesPerSecond: (delta * 1000) / Math.max(1, options.delayMs),
						},
					});
				}
				await delay();
				publish({ _tag: "Ready", version });
			},
			installUpdate: async () => {
				if (status._tag !== "Ready") throw new Error("No update is ready to install.");
				const { failure } = options;
				publish({ _tag: "Installing", version: status.version });
				await delay();
				if (failure === "Install") {
					publish({ _tag: "Idle" });
					throw new Error("Simulated installation failure");
				}
				return new Promise<never>(() => {});
			},
			onUpdateStatusChange: (listener) => {
				listeners.add(listener);
				return () => listeners.delete(listener);
			},
		} satisfies Pick<
			LiteElectronApi,
			| "getUpdateStatus"
			| "checkForUpdates"
			| "downloadUpdate"
			| "installUpdate"
			| "onUpdateStatusChange"
		>;
		demo = { client, options };
		client.setQueryData(guiSettingsQueryOptions.queryKey, { version: 1, autoUpdate: false });
		client.setQueryData(["updateStatus"], status);
		window.lite = { ...window.lite, ...api };
		return () => {
			for (const timer of timers) clearTimeout(timer);
			listeners.clear();
			void client.cancelQueries();
		};
	},
});

export const Playground = meta.story({});

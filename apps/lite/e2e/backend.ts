import type { ElectronApplication } from "@playwright/test";

/** The app's main process, answering the renderer's IPC calls instead of the real backend. */
export class Backend {
	private readonly electronApp: ElectronApplication;

	constructor(electronApp: ElectronApplication) {
		this.electronApp = electronApp;
	}

	/** Answer the named calls with fixed values. */
	stub(responses: Record<string, unknown>): Promise<void> {
		return this.electronApp.evaluate(({ ipcMain }, responses) => {
			for (const [channel, value] of Object.entries(responses)) {
				ipcMain.removeHandler(channel);
				ipcMain.handle(channel, () => value);
			}
		}, responses);
	}

	/** Swallow calls on `channel`; the returned function reads the arguments of each so far. */
	async record(channel: string): Promise<() => Promise<Array<Array<unknown>>>> {
		await this.electronApp.evaluate(({ ipcMain }, channel) => {
			const calls: Array<Array<unknown>> = [];
			(globalThis as { recordedCalls?: Record<string, typeof calls> }).recordedCalls = {
				...(globalThis as { recordedCalls?: Record<string, typeof calls> }).recordedCalls,
				[channel]: calls,
			};
			ipcMain.removeHandler(channel);
			ipcMain.handle(channel, (_event, ...args: Array<unknown>) => {
				calls.push(args);
			});
		}, channel);
		return () =>
			this.electronApp.evaluate(
				(_electron, channel) =>
					(globalThis as { recordedCalls?: Record<string, Array<Array<unknown>>> }).recordedCalls?.[
						channel
					] ?? [],
				channel,
			);
	}

	/** Hold calls on `channel` until the returned function answers them. */
	async answerLater(channel: string): Promise<(value: unknown) => Promise<void>> {
		const answer = await this.electronApp.evaluateHandle(({ ipcMain }, channel) => {
			const { promise, resolve } = Promise.withResolvers<unknown>();
			ipcMain.removeHandler(channel);
			ipcMain.handle(channel, () => promise);
			return resolve;
		}, channel);
		return async (value) => {
			await answer.evaluate((resolve, value) => resolve(value), value);
			await answer.dispose();
		};
	}

	/** Push an event to the window, as the main process does on its own. */
	send(channel: string, payload: unknown): Promise<void> {
		return this.electronApp.evaluate(
			({ BrowserWindow }, { channel, payload }) => {
				BrowserWindow.getAllWindows()[0]?.webContents.send(channel, payload);
			},
			{ channel, payload },
		);
	}
}

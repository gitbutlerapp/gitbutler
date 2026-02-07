/**
 * Working Files Broadcast
 *
 * Thin frontend wrapper that tells the Rust backend to start/stop
 * broadcasting the user's modified files to the project IRC channel.
 * All file tracking, delta computation, debouncing, and message sending
 * is handled by the backend.
 */

import { InjectionToken } from "@gitbutler/core/context";
import { IRC_CONNECTION_ID } from "$lib/irc/ircApiService";
import type { IBackend } from "$lib/backend/backend";

export const WORKING_FILES_BROADCAST = new InjectionToken<WorkingFilesBroadcast>(
	"WorkingFilesBroadcast",
);

export class WorkingFilesBroadcast {
	private projectId: string | undefined = $state();
	channel: string | undefined = $state();

	constructor(private backend: IBackend) {}

	async start(projectId: string, channel: string): Promise<void> {
		this.projectId = projectId;
		this.channel = channel;
		await this.backend.invoke("irc_start_working_files_broadcast", {
			projectId,
			connectionId: IRC_CONNECTION_ID,
			channel,
		});
	}

	async stop(): Promise<void> {
		if (!this.projectId) return;
		await this.backend
			.invoke("irc_stop_working_files_broadcast", {
				projectId: this.projectId,
			})
			.catch(() => {});
		this.projectId = undefined;
		// Note: `channel` is managed separately by the personal IRC effect
		// for FileList working-files queries, so we don't clear it here.
	}
}

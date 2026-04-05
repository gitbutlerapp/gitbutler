import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { BackendApi } from "$lib/state/clientState.svelte";
import type { Action } from "@gitbutler/core/api";

export const ACTION_SERVICE = new InjectionToken<ActionService>("ActionService");

export class ActionService {
	constructor(
		private backendApi: BackendApi,
		private backend: IBackend,
	) {}

	get autoCommit() {
		return this.backendApi.endpoints.autoCommit.useMutation();
	}

	listenForAutoCommit(projectId: string, listen: (event: Action.AutoCommitEvent) => void) {
		const unlisten = this.backend.listen(`project://${projectId}/auto-commit`, (event) => {
			const payload = event.payload as Action.AutoCommitEvent;
			listen(payload);
		});

		return unlisten;
	}

	get branchChanges() {
		return this.backendApi.endpoints.autoBranchChanges.useMutation();
	}

	get bot() {
		return this.backendApi.endpoints.bot.useMutation();
	}
}

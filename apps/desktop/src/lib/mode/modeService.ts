import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";

export const MODE_SERVICE = new InjectionToken<ModeService>("ModeService");

export class ModeService {
	constructor(private backendApi: BackendApi) {}

	get enterEditMode() {
		return this.backendApi.endpoints.enterEditMode.mutate;
	}

	get abortEditAndReturnToWorkspace() {
		return this.backendApi.endpoints.abortEditAndReturnToWorkspace.mutate;
	}

	get abortEditAndReturnToWorkspaceMutation() {
		return this.backendApi.endpoints.abortEditAndReturnToWorkspace.useMutation();
	}

	get saveEditAndReturnToWorkspace() {
		return this.backendApi.endpoints.saveEditAndReturnToWorkspace.mutate;
	}

	get saveEditAndReturnToWorkspaceMutation() {
		return this.backendApi.endpoints.saveEditAndReturnToWorkspace.useMutation();
	}

	get initialEditModeState() {
		return this.backendApi.endpoints.initialEditModeState.useQuery;
	}

	get changesSinceInitialEditState() {
		return this.backendApi.endpoints.changesSinceInitialEditState.useQuery;
	}

	mode(projectId: string) {
		return this.backendApi.endpoints.headAndMode.useQuery(
			{ projectId },
			{ transform: (response) => response.operatingMode },
		);
	}

	/**
	 * Force-fetch the current mode, bypassing the cache. This updates the
	 * cache so that reactive subscribers see the new value immediately.
	 */
	async fetchMode(projectId: string) {
		return await this.backendApi.endpoints.headAndMode.fetch({ projectId }, { forceRefetch: true });
	}

	head(projectId: string) {
		return this.backendApi.endpoints.headSha.useQuery(
			{ projectId },
			{ transform: (response) => response.headSha },
		);
	}
}

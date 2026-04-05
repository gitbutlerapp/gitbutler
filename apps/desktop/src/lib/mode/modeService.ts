import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";

export type { EditModeMetadata, OutsideWorkspaceMetadata, Mode } from "$lib/mode/modeEndpoints";

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

	head(projectId: string) {
		return this.backendApi.endpoints.headSha.useQuery(
			{ projectId },
			{ transform: (response) => response.headSha },
		);
	}
}

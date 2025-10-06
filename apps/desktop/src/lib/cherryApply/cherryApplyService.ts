import { InjectionToken } from '@gitbutler/core/context';
import type { BackendApi } from '$lib/state/clientState.svelte';

export type CherryApplyStatus =
	| {
			type: 'causesWorkspaceConflict';
	  }
	| {
			type: 'lockedToStack';
			subject: string;
	  }
	| {
			type: 'applicableToAnyStack';
	  }
	| {
			type: 'noStacks';
	  };

export const CHERRY_APPLY_SERVICE = new InjectionToken<CherryApplyService>('CherryApplyService');

export class CherryApplyService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get status() {
		return this.api.endpoints.cherryApplyStatus.useQuery;
	}
}

function injectEndpoints(backendApi: BackendApi) {
	return backendApi.injectEndpoints({
		endpoints: (build) => ({
			cherryApplyStatus: build.query<CherryApplyStatus, { projectId: string; subject: string }>(
				{
					extraOptions: { command: 'cherry_apply_status' },
					query: (args) => args
				}
			)
		})
	});
}

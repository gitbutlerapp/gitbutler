import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";

export const CHERRY_APPLY_SERVICE = new InjectionToken<CherryApplyService>("CherryApplyService");

export class CherryApplyService {
	constructor(private backendApi: BackendApi) {}

	get status() {
		return this.backendApi.endpoints.cherryApplyStatus.useQuery;
	}

	get apply() {
		return this.backendApi.endpoints.cherryApply.useMutation;
	}
}

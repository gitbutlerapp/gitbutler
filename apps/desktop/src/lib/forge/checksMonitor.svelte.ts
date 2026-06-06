import { providesItem, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { ChecksStatus } from "$lib/forge/interface/types";
import type { BackendApi } from "$lib/state/backendApi";
import type { QueryOptions } from "$lib/state/butlerModule";
import type { CiCheck } from "@gitbutler/but-sdk";

export const CHECKS_MONITOR = new InjectionToken<ChecksMonitor>("ChecksMonitor");

export class ChecksMonitor {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get(projectId: string, branchName: string, options?: QueryOptions) {
		return this.api.endpoints.listCiChecks.useQuery(
			{ projectId, reference: branchName },
			{
				transform: (result) => parseChecks(result),
				...options,
			},
		);
	}

	async fetch(projectId: string, branchName: string, options?: QueryOptions) {
		return await this.api.endpoints.listCiChecks.fetch(
			{ projectId, reference: branchName },
			{
				transform: (result) => parseChecks(result),
				...options,
			},
		);
	}
}

// `cancelled` runs are dropped upstream in the Rust client.
function parseChecks(checks: CiCheck[]): ChecksStatus | null {
	if (checks.length === 0) return null;

	let failedCount = 0;
	let actionRequiredCount = 0;
	let allCompleted = true;
	const failedNames: string[] = [];
	const startTimestamps: number[] = [];

	for (const check of checks) {
		if (check.startedAt) {
			startTimestamps.push(new Date(check.startedAt).getTime());
		}

		if (typeof check.status === "string") {
			allCompleted = false;
		} else {
			const conclusion = check.status.complete.conclusion;
			if (conclusion === "failure") {
				failedCount++;
				failedNames.push(check.name);
			} else if (conclusion === "actionRequired") {
				actionRequiredCount++;
			}
		}
	}

	// Any failure short-circuits the badge to "done".
	const completed = failedCount > 0 || allCompleted;
	const success = failedCount === 0 && actionRequiredCount === 0 && completed;
	const startedAt =
		startTimestamps.length > 0 ? new Date(Math.min(...startTimestamps)).toISOString() : null;

	return { startedAt, completed, success, failedChecks: failedNames };
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listCiChecks: build.query<CiCheck[], { projectId: string; reference: string }>({
				extraOptions: { command: "list_ci_checks" },
				query: ({ projectId, reference }) => ({
					projectId,
					reference,
					// Polling drives freshness; bypass the DB cache so we always
					// see the latest GitHub state. The cache stays useful for
					// read-only consumers (e.g. `but status` without `-r`).
					cacheConfig: "noCache",
				}),
				providesTags: (_result, _error, args) => [...providesItem(ReduxTag.Checks, args.reference)],
			}),
		}),
	});
}

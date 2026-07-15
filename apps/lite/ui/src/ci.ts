import type { CiCheck, CiConclusion, CiStatus, ForgeInfo } from "@gitbutler/but-sdk";

export const prForgeUrl = (prNo: number, forge: ForgeInfo): string =>
	`${forge.baseUrl}${forge.prUrlPath}${prNo}`;

// TODO: We're missing equivalent to prUrlPath from SDK for forge-agnostic CI summary URL.
export const ciChecksSummaryUrl = (prNo: number, forge: ForgeInfo): string | null =>
	forge.name === "github" ? `${prForgeUrl(prNo, forge)}/checks` : null;

export type AggregateCIStatus =
	| "success"
	| "failure"
	| "cancelled"
	| "action_required"
	| "in_progress"
	| "unknown";

type SDKStatus = Extract<CiStatus, string> | CiConclusion;

export type AggregateCIChecks = {
	status: AggregateCIStatus;
	total: number;
} & Record<SDKStatus, Array<CiCheck>>;

export const aggregateCIChecks = (checks: Array<CiCheck>): AggregateCIChecks | null => {
	if (checks.length === 0) return null;

	const aggregate: AggregateCIChecks = {
		status: "unknown",
		total: checks.length,
		failure: [],
		timedOut: [],
		actionRequired: [],
		cancelled: [],
		inProgress: [],
		queued: [],
		success: [],
		neutral: [],
		skipped: [],
		unknown: [],
	};

	for (const check of checks) {
		aggregate[
			typeof check.status === "string" ? check.status : check.status.complete.conclusion
		].push(check);
	}

	if (aggregate.failure.length > 0 || aggregate.timedOut.length > 0) aggregate.status = "failure";
	else if (aggregate.actionRequired.length > 0) aggregate.status = "action_required";
	else if (aggregate.cancelled.length > 0) aggregate.status = "cancelled";
	else if (aggregate.inProgress.length > 0 || aggregate.queued.length > 0)
		aggregate.status = "in_progress";
	else if (
		aggregate.success.length > 0 ||
		aggregate.neutral.length > 0 ||
		aggregate.skipped.length > 0
	)
		aggregate.status = "success";

	return aggregate;
};

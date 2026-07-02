import type { CiCheck, CiConclusion, CiStatus, ForgeInfo } from "@gitbutler/but-sdk";
import { prForgeUrl } from "./pr.ts";

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
} & Record<SDKStatus, number>;

export const aggregateCIChecks = (checks: Array<CiCheck>): AggregateCIChecks | null => {
	if (checks.length === 0) return null;

	const aggregate: AggregateCIChecks = {
		status: "unknown",
		total: checks.length,
		failure: 0,
		timedOut: 0,
		actionRequired: 0,
		cancelled: 0,
		inProgress: 0,
		queued: 0,
		success: 0,
		neutral: 0,
		skipped: 0,
		unknown: 0,
	};

	for (const check of checks)
		aggregate[typeof check.status === "string" ? check.status : check.status.complete.conclusion]++;

	if (aggregate.failure > 0 || aggregate.timedOut > 0) aggregate.status = "failure";
	else if (aggregate.actionRequired > 0) aggregate.status = "action_required";
	else if (aggregate.cancelled > 0) aggregate.status = "cancelled";
	else if (aggregate.inProgress > 0 || aggregate.queued > 0) aggregate.status = "in_progress";
	else if (aggregate.success > 0 || aggregate.neutral > 0 || aggregate.skipped > 0)
		aggregate.status = "success";

	return aggregate;
};

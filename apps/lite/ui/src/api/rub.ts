import { UICommitCreateResult } from "@gitbutler/but-sdk";
import { Match } from "effect";
import {
	type AssignHunkParams,
	type CommitAmendParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
} from "#electron/ipc.ts";

// TODO: replace with generated type when it becomes available
type CommitUncommitParams = {
	projectId: string;
	commitId: string;
	assignTo: string | null;
};

// TODO: replace with generated type when it becomes available
type CommitSquashParams = {
	projectId: string;
	sourceCommitId: string;
	destinationCommitId: string;
};

export type RubOperation =
	| ({ _tag: "AssignHunk" } & Omit<AssignHunkParams, "projectId">)
	| ({ _tag: "CommitAmend" } & Omit<CommitAmendParams, "projectId">)
	| ({ _tag: "CommitMoveChangesBetween" } & Omit<CommitMoveChangesBetweenParams, "projectId">)
	| ({ _tag: "CommitSquash" } & Omit<CommitSquashParams, "projectId">)
	| ({ _tag: "CommitUncommit" } & Omit<CommitUncommitParams, "projectId">)
	| ({ _tag: "CommitUncommitChanges" } & Omit<CommitUncommitChangesParams, "projectId">);

export type RubParams = {
	projectId: string;
	operation: RubOperation;
};

/** @public */
export type RubResult = {
	replacedCommits?: Record<string, string>;
	newCommit?: string | null;
	amendedCommitId?: string;
	rejectedChanges?: UICommitCreateResult["rejectedChanges"];
};

// In the future this may be implemented as a single API endpoint on the backend.
export const rub = async ({ projectId, operation }: RubParams): Promise<RubResult> =>
	Match.value(operation).pipe(
		Match.tag("AssignHunk", async (operation): Promise<RubResult> => {
			await window.lite.assignHunk({
				projectId,
				assignments: operation.assignments,
			});
			return {};
		}),
		Match.tag("CommitAmend", async (operation): Promise<RubResult> => {
			const response = await window.lite.commitAmend({
				projectId,
				commitId: operation.commitId,
				changes: operation.changes,
			});
			return {
				replacedCommits: response.replacedCommits,
				newCommit: response.newCommit ?? null,
				amendedCommitId: operation.commitId,
				rejectedChanges: response.rejectedChanges,
			};
		}),
		Match.tag("CommitUncommitChanges", async (operation): Promise<RubResult> => {
			const response = await window.lite.commitUncommitChanges({
				projectId,
				commitId: operation.commitId,
				assignTo: operation.assignTo,
				changes: operation.changes,
			});
			return {
				replacedCommits: response.replacedCommits,
			};
		}),
		Match.tag("CommitMoveChangesBetween", async (operation): Promise<RubResult> => {
			const response = await window.lite.commitMoveChangesBetween({
				projectId,
				sourceCommitId: operation.sourceCommitId,
				destinationCommitId: operation.destinationCommitId,
				changes: operation.changes,
			});
			return { replacedCommits: response.replacedCommits };
		}),
		// TODO: implement when API is ready
		Match.tag("CommitUncommit", async (): Promise<RubResult> => {
			throw new Error("Uncommitting has not been implemented yet.");
		}),
		// TODO: implement when API is ready
		Match.tag("CommitSquash", async (): Promise<RubResult> => {
			throw new Error("Squashing has not been implemented yet.");
		}),
		Match.exhaustive,
	);

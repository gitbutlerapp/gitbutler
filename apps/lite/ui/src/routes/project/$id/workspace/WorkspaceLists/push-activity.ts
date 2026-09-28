import type { PayloadFor } from "#electron/ipc.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import type { Segment } from "@gitbutler/but-sdk";
import { useMutationState } from "@tanstack/react-query";
import { useMemo } from "react";

export type PushActivity = "idle" | "blocked" | "pushing";

export const usePendingPushBranches = (projectId: string): Set<string> => {
	const branches = useMutationState({
		filters: {
			mutationKey: [projectId, "workspaceBranchAndAncestorsPush"],
			status: "pending",
		},
		select: (mutation) =>
			(mutation.state.variables as PayloadFor<"workspaceBranchAndAncestorsPush">).branch,
	});
	return useMemo(() => new Set(branches), [branches]);
};

export const pushActivities = (
	segments: ReadonlyArray<Segment>,
	pendingPushBranches: ReadonlySet<string>,
): Array<PushActivity> => {
	const topmostPending = segments.findIndex(
		(segment) =>
			segment.refName !== null &&
			pendingPushBranches.has(decodeBytes(segment.refName.fullNameBytes)),
	);
	return segments.map((_, index) =>
		topmostPending === -1 ? "idle" : index >= topmostPending ? "pushing" : "blocked",
	);
};

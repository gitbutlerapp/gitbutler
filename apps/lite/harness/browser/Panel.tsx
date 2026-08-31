import { useQueries, useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { Match } from "effect";
import type { FC } from "react";
import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
} from "#ui/api/queries.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { buildAppliedAddressSpace } from "#ui/routes/project/$id/workspace/applied-address-space.ts";
import { buildUncommittedFileRows } from "#ui/routes/project/$id/workspace/file-row.ts";
import { fileTreeAddressSpace } from "#ui/routes/project/$id/workspace/file-tree.ts";
import { useFileDisplayMode } from "#ui/routes/project/$id/workspace/useFileDisplayMode.ts";
import { WorkspaceLists } from "#ui/routes/project/$id/workspace/WorkspaceLists/WorkspaceLists.tsx";
import { useReviewActivityInbox } from "#ui/review-notifications.ts";
import { useStampReviewsSeen } from "#ui/review-seen.ts";
import { useAppSelector } from "#ui/store.ts";
import { setCursor } from "#ui/use-cursor.ts";
import styles from "./Panel.module.css";

/**
 * The workspace route's component in the panel: the app's workspace lists,
 * fed by the same derivations Page runs, minus its Details pane and chrome.
 */
export const Panel: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	// The panel toasts review activity like the app does — same detector, same
	// declarations — so the harness rig can exercise the flow end to end.
	useReviewActivityInbox(projectId);
	useStampReviewsSeen(projectId);
	const pendingOperation = useAppSelector((state) =>
		projectSlice.selectors.selectPendingOperation(state, projectId),
	);
	const absorptionPlanTarget = Match.value(pendingOperation).pipe(
		Match.tags({ Absorb: ({ sourceTarget }) => sourceTarget }),
		Match.orElse(() => null),
	);
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const [absorptionPlanQuery] = useQueries({
		queries: (absorptionPlanTarget ? [absorptionPlanTarget] : []).map((target) =>
			absorptionPlanQueryOptions({ projectId, target }),
		),
	});
	const absorptionTargetCommitIds = new Set(
		absorptionPlanQuery?.data?.map(({ commitId }) => commitId),
	);

	const foldedSegments = useAppSelector((state) =>
		projectSlice.selectors.selectFoldedSegments(state, projectId),
	);
	const appliedAddressSpace = buildAppliedAddressSpace({
		headInfo,
		pendingOperation,
		absorptionTargetCommitIds,
		foldedSegments,
	});

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const uncommittedFilesFilter = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesFilter(state, projectId),
	);
	const uncommittedFilesDisplayMode = useFileDisplayMode();
	const uncommittedFilesCollapsedDirectories = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesCollapsedDirectories(state, projectId),
	);
	const uncommittedFileRows = buildUncommittedFileRows({
		worktreeChanges,
		filter: uncommittedFilesFilter,
		mode: uncommittedFilesDisplayMode,
		collapsedDirectories: uncommittedFilesCollapsedDirectories,
		// The panel has no recency toggle, so it lists in path order.
		recentFirst: false,
	});
	const uncommittedAddressSpace = fileTreeAddressSpace(uncommittedFileRows);

	// Moving the uncommitted cursor is what highlights the row and makes the
	// file list keyboard-navigable (arrow keys route through the same handler).
	// The panel has no details pane yet, so the diff cursor stays untouched.
	const onActiveFileSelection = (selection: string): void => {
		setCursor("uncommitted", selection);
	};

	// The app registers these focus hotkeys in Page.tsx, which the panel never
	// renders — so the panel owns its own copies: 1 focuses the uncommitted
	// files list, 2 the stacks/branches list (the sidebar scope).
	useHotkeys([
		{
			hotkey: "1",
			callback: () => focusScope("uncommitted-files"),
		},
		{
			hotkey: "2",
			callback: () => focusScope("sidebar"),
		},
	]);

	return (
		<div className={styles.panel}>
			<WorkspaceLists
				addressSpace={appliedAddressSpace}
				uncommittedAddressSpace={uncommittedAddressSpace}
				absorptionTargetCommitIds={absorptionTargetCommitIds}
				projectId={projectId}
				// The panel has no details pane yet; activating a file only
				// moves the list cursor (see onActiveFileSelection above).
				onActiveFileSelection={onActiveFileSelection}
			/>
		</div>
	);
};

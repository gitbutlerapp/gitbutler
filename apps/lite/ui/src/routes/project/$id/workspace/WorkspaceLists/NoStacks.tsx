import { branchListQueryOptions } from "#ui/api/queries.ts";
import { unappliedStacks } from "#ui/branch.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { EmptyState } from "#ui/components/EmptyState.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { focusScope } from "#ui/focus-scopes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { setPage } from "#ui/use-cursor.ts";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";
import type { NewBranchActions } from "../useNewBranch.ts";

/**
 * The stacks panel with nothing applied, which is two states rather than one.
 *
 * A project with no branches at all needs no rescue: committing creates one on
 * its own, so the panel says so and the button beside it is only a shortcut. A
 * project whose branches are simply elsewhere is a different question — the
 * work exists and the panel is the wrong place to be looking — so there the
 * highlight goes on the way back to them, and the count is what makes that
 * worth pressing.
 */
export const NoStacks: FC<{ projectId: string; newBranch: NewBranchActions }> = ({
	projectId,
	newBranch,
}) => {
	// The same filters the branches page lists under, so the count promises
	// exactly what "See all" then shows.
	const filters = useAppSelector((state) =>
		projectSlice.selectors.selectBranchFilters(state, projectId),
	);
	const { data: unappliedBranchCount } = useQuery({
		...branchListQueryOptions(projectId),
		// Derived in `select` so react-query caches it against the branch list
		// rather than recounting every render.
		select: (listedStacks) =>
			unappliedStacks(listedStacks, filters).reduce(
				(count, stack) => count + stack.branches.length,
				0,
			),
	});

	// Not loaded is not the same as nothing to report: rendering early would
	// flash the fresh-project wording at every project that has branches.
	if (unappliedBranchCount === undefined) return null;

	const hasBranchesElsewhere = unappliedBranchCount > 0;

	return (
		<EmptyState
			illustration="cactus"
			title={hasBranchesElsewhere ? "Your workspace is empty" : "No branches yet"}
			description={
				hasBranchesElsewhere
					? `You have ${unappliedBranchCount} ${unappliedBranchCount === 1 ? "branch" : "branches"} to pick from`
					: "Your first commit will start one"
			}
		>
			{hasBranchesElsewhere && (
				<button
					type="button"
					className={getButtonClassName({ variant: "gray" })}
					onClick={() => {
						setPage("branches");
						focusScope("sidebar");
					}}
				>
					See all
					<Icon name="list" />
				</button>
			)}
			<button
				type="button"
				className={getButtonClassName({ variant: "outline" })}
				disabled={!newBranch.canCreateInWorkspace}
				onClick={newBranch.createInWorkspace}
			>
				New branch
				<Icon name="plus" />
			</button>
		</EmptyState>
	);
};

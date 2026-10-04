import type { MeshGrouping } from "#ui/interface/state.ts";
import type { Commit } from "@gitbutler/but-sdk";
import type { MeshBranch, MeshCheckout, MeshMachine, MeshRepo } from "./useMeshTree.ts";

type RowBase = {
	key: string;
	parentKey: string | null;
	depth: number;
	positionInSet: number;
	setSize: number;
	/** Undefined where the row has nothing to fold. */
	folded?: boolean;
};

export type MeshRow = RowBase &
	(
		| { _tag: "Machine"; machine: MeshMachine }
		| { _tag: "Repo"; repo: MeshRepo }
		/** Named by whichever of repo and machine its group doesn't already say. */
		| { _tag: "Checkout"; checkout: MeshCheckout; name: string }
		| { _tag: "Uncommitted"; checkout: MeshCheckout }
		| { _tag: "Branch"; checkout: MeshCheckout; branch: MeshBranch }
		| { _tag: "Commit"; checkout: MeshCheckout; commit: Commit }
	);

/**
 * The tree's visible rows in reading order, flattened as the files tree's are: depth is reported
 * rather than nested, and the order is what the arrow keys walk.
 */
export const buildMeshRows = ({
	machines,
	repos,
	grouping,
	toggled,
}: {
	machines: Array<MeshMachine>;
	repos: Array<MeshRepo>;
	grouping: MeshGrouping;
	toggled: Record<string, true>;
}): Array<MeshRow> => {
	const rows: Array<MeshRow> = [];
	const isFolded = (key: string, startsFolded: boolean) =>
		toggled[key] === true ? !startsFolded : startsFolded;

	const pushCheckouts = (
		parentKey: string,
		checkouts: Array<MeshCheckout>,
		name: (checkout: MeshCheckout) => string,
	) => {
		checkouts.forEach((checkout, index) => {
			const key = `checkout:${checkout.machine}:${checkout.projectId}`;
			const folded = isFolded(key, false);
			rows.push({
				_tag: "Checkout",
				key,
				parentKey,
				depth: 1,
				positionInSet: index + 1,
				setSize: checkouts.length,
				folded,
				checkout,
				name: name(checkout),
			});
			if (folded) return;

			const hasUncommitted = checkout.uncommittedFiles > 0;
			const setSize = checkout.branches.length + (hasUncommitted ? 1 : 0);
			if (hasUncommitted) {
				rows.push({
					_tag: "Uncommitted",
					key: `${key}:uncommitted`,
					parentKey: key,
					depth: 2,
					positionInSet: 1,
					setSize,
					checkout,
				});
			}
			checkout.branches.forEach((branch, branchIndex) => {
				const branchKey = `${key}:branch:${branch.name}`;
				const branchFolded = isFolded(branchKey, true);
				rows.push({
					_tag: "Branch",
					key: branchKey,
					parentKey: key,
					depth: 2,
					positionInSet: branchIndex + 1 + (hasUncommitted ? 1 : 0),
					setSize,
					folded: branchFolded,
					checkout,
					branch,
				});
				if (branchFolded) return;
				branch.commits.forEach((commit, commitIndex) => {
					rows.push({
						_tag: "Commit",
						key: `${branchKey}:commit:${commit.id}`,
						parentKey: branchKey,
						depth: 3,
						positionInSet: commitIndex + 1,
						setSize: branch.commits.length,
						checkout,
						commit,
					});
				});
			});
		});
	};

	if (grouping === "machines") {
		machines.forEach((machine, index) => {
			const key = `machine:${machine.name}`;
			const folded = isFolded(key, false);
			rows.push({
				_tag: "Machine",
				key,
				parentKey: null,
				depth: 0,
				positionInSet: index + 1,
				setSize: machines.length,
				folded,
				machine,
			});
			if (!folded) pushCheckouts(key, machine.checkouts, (checkout) => checkout.repo);
		});
	} else {
		repos.forEach((repo, index) => {
			const key = `repo:${repo.projectId}`;
			const folded = isFolded(key, false);
			rows.push({
				_tag: "Repo",
				key,
				parentKey: null,
				depth: 0,
				positionInSet: index + 1,
				setSize: repos.length,
				folded,
				repo,
			});
			if (!folded) pushCheckouts(key, repo.checkouts, (checkout) => checkout.machine);
		});
	}

	return rows;
};

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
		/** `uncommitted` when it's another machine's published uncommitted changes. */
		| { _tag: "Commit"; checkout: MeshCheckout; commit: Commit; uncommitted: boolean }
	);

export const checkoutKey = (machine: string, projectId: string) =>
	`checkout:${machine}:${projectId}`;

export const repoKey = (projectId: string) => `repo:${projectId}`;

/**
 * The tree's visible rows in reading order, flattened as the files tree's are: depth is reported
 * rather than nested, and the order is what the arrow keys walk.
 */
export const buildMeshRows = ({
	machines,
	repos,
	grouping,
	unfolded,
}: {
	machines: Array<MeshMachine>;
	repos: Array<MeshRepo>;
	grouping: MeshGrouping;
	unfolded: Record<string, true>;
}): Array<MeshRow> => {
	const rows: Array<MeshRow> = [];
	const isFolded = (key: string) => unfolded[key] !== true;

	/**
	 * A checkout's uncommitted changes and branches, under `parentKey` at `depth`. Their keys stay
	 * the checkout's, so folds hold whether or not the checkout has a row of its own.
	 */
	const pushContents = (checkout: MeshCheckout, parentKey: string, depth: number) => {
		const key = checkoutKey(checkout.machine, checkout.projectId);
		const hasUncommitted = checkout.uncommittedFiles > 0;
		const setSize = checkout.branches.length + (hasUncommitted ? 1 : 0);
		if (hasUncommitted) {
			rows.push({
				_tag: "Uncommitted",
				key: `${key}:uncommitted`,
				parentKey,
				depth,
				positionInSet: 1,
				setSize,
				checkout,
			});
		}
		checkout.branches.forEach((branch, branchIndex) => {
			const branchKey = `${key}:branch:${branch.name}`;
			const branchFolded = isFolded(branchKey);
			rows.push({
				_tag: "Branch",
				key: branchKey,
				parentKey,
				depth,
				positionInSet: branchIndex + 1 + (hasUncommitted ? 1 : 0),
				setSize,
				folded: branchFolded,
				checkout,
				branch,
			});
			if (branchFolded) return;
			const commits = branch.uncommitted ? [branch.uncommitted, ...branch.commits] : branch.commits;
			commits.forEach((commit, commitIndex) => {
				rows.push({
					_tag: "Commit",
					key: `${branchKey}:commit:${commit.id}`,
					parentKey: branchKey,
					depth: depth + 1,
					positionInSet: commitIndex + 1,
					setSize: commits.length,
					checkout,
					commit,
					uncommitted: commit === branch.uncommitted,
				});
			});
		});
	};

	const pushCheckouts = (
		parentKey: string,
		checkouts: Array<MeshCheckout>,
		name: (checkout: MeshCheckout) => string,
	) => {
		checkouts.forEach((checkout, index) => {
			const key = checkoutKey(checkout.machine, checkout.projectId);
			const folded = isFolded(key);
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
			if (!folded) pushContents(checkout, key, 2);
		});
	};

	if (grouping === "machines") {
		machines.forEach((machine, index) => {
			const key = `machine:${machine.name}`;
			const folded = isFolded(key);
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
			const key = repoKey(repo.projectId);
			const folded = isFolded(key);
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
			if (folded) return;
			// A repo only this machine has needs no row saying so.
			const [only, ...others] = repo.checkouts;
			if (only?.isThisMachine === true && others.length === 0) pushContents(only, key, 1);
			else pushCheckouts(key, repo.checkouts, (checkout) => checkout.machine);
		});
	}

	return rows;
};

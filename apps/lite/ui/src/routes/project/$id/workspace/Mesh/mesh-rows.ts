import type { MeshGrouping } from "#ui/interface/state.ts";
import type { Commit } from "@gitbutler/but-sdk";
import type {
	MeshBranch,
	MeshCheckout,
	MeshMachine,
	MeshRepo,
	MeshWorktree,
} from "./useMeshTree.ts";

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
		/** With `worktree`, that linked worktree's files rather than the main checkout's. */
		| { _tag: "Uncommitted"; checkout: MeshCheckout; worktree?: MeshWorktree }
		/** With `branch`, its only branch, which then has no row of its own. */
		| { _tag: "Worktree"; checkout: MeshCheckout; worktree: MeshWorktree; branch?: MeshBranch }
		| { _tag: "Branch"; checkout: MeshCheckout; branch: MeshBranch }
		/** `uncommitted` when it's another machine's published uncommitted changes. */
		| {
				_tag: "Commit";
				checkout: MeshCheckout;
				commit: Commit;
				/** The linked worktree whose branch it's on. */
				worktree?: string;
				uncommitted: boolean;
		  }
	);

export const checkoutKey = (machine: string, projectId: string) =>
	`checkout:${machine}:${projectId}`;

export const repoKey = (projectId: string) => `repo:${projectId}`;

/** A worktree of the checkout keyed `checkout`; with an empty `name`, the prefix of them all. */
export const worktreeKey = (checkout: string, name: string) => `${checkout}:worktree:${name}`;

/**
 * The tree's visible rows in reading order, flattened as the files tree's are: depth is reported
 * rather than nested, and the order is what the arrow keys walk.
 */
export const buildMeshRows = ({
	machines,
	repos,
	grouping,
	unfolded,
	order,
}: {
	machines: Array<MeshMachine>;
	repos: Array<MeshRepo>;
	grouping: MeshGrouping;
	unfolded: Record<string, true>;
	/** Row keys by position, to keep rows where they were while the pointer is over them. */
	order?: ReadonlyMap<string, number>;
}): Array<MeshRow> => {
	const rows: Array<MeshRow> = [];
	const isFolded = (key: string) => unfolded[key] !== true;
	const inOrder = <T>(items: Array<T>, keyOf: (item: T) => string): Array<T> =>
		order === undefined
			? items
			: items.toSorted(
					(a, b) =>
						(order.get(keyOf(a)) ?? Number.MAX_SAFE_INTEGER) -
						(order.get(keyOf(b)) ?? Number.MAX_SAFE_INTEGER),
				);

	/** Branches keyed under `base`, from `positionInSet` on in a set of `setSize`. */
	const pushBranches = (
		checkout: MeshCheckout,
		branches: Array<MeshBranch>,
		{
			base,
			parentKey,
			depth,
			from,
			setSize,
		}: { base: string; parentKey: string; depth: number; from: number; setSize: number },
	) => {
		branches.forEach((branch, branchIndex) => {
			const branchKey = `${base}:branch:${branch.name}`;
			const branchFolded = isFolded(branchKey);
			rows.push({
				_tag: "Branch",
				key: branchKey,
				parentKey,
				depth,
				positionInSet: from + branchIndex,
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
					worktree: branch.worktree,
					uncommitted: commit === branch.uncommitted,
				});
			});
		});
	};

	/**
	 * A checkout's uncommitted changes, branches and linked worktrees, under `parentKey` at
	 * `depth`. Their keys stay the checkout's, so folds hold whether or not it has a row.
	 */
	const pushContents = (checkout: MeshCheckout, parentKey: string, depth: number) => {
		const key = checkoutKey(checkout.machine, checkout.projectId);
		const hasUncommitted = checkout.uncommittedFiles > 0;
		const before = hasUncommitted ? 1 : 0;
		const setSize = before + checkout.branches.length + checkout.worktrees.length;
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
		pushBranches(checkout, checkout.branches, {
			base: key,
			parentKey,
			depth,
			from: before + 1,
			setSize,
		});
		checkout.worktrees.forEach((worktree, index) => {
			const wtKey = worktreeKey(key, worktree.name);
			const folded = isFolded(wtKey);
			// A worktree with one branch is one row, its commits right under it.
			const [only, ...others] = worktree.branches;
			const branch = only !== undefined && others.length === 0 ? only : undefined;
			rows.push({
				_tag: "Worktree",
				key: wtKey,
				parentKey,
				depth,
				positionInSet: before + checkout.branches.length + index + 1,
				setSize,
				folded,
				checkout,
				worktree,
				branch,
			});
			if (folded) return;
			const dirty = worktree.files.length > 0;
			const commits = branch === undefined ? [] : branch.commits;
			const innerSize =
				(dirty ? 1 : 0) + (branch === undefined ? worktree.branches.length : commits.length);
			if (dirty) {
				rows.push({
					_tag: "Uncommitted",
					key: `${wtKey}:uncommitted`,
					parentKey: wtKey,
					depth: depth + 1,
					positionInSet: 1,
					setSize: innerSize,
					checkout,
					worktree,
				});
			}
			if (branch !== undefined) {
				commits.forEach((commit, commitIndex) => {
					rows.push({
						_tag: "Commit",
						key: `${wtKey}:commit:${commit.id}`,
						parentKey: wtKey,
						depth: depth + 1,
						positionInSet: (dirty ? 2 : 1) + commitIndex,
						setSize: innerSize,
						checkout,
						commit,
						worktree: worktree.name,
						uncommitted: false,
					});
				});
				return;
			}
			pushBranches(checkout, worktree.branches, {
				base: wtKey,
				parentKey: wtKey,
				depth: depth + 1,
				from: dirty ? 2 : 1,
				setSize: innerSize,
			});
		});
	};

	const pushCheckouts = (
		parentKey: string,
		checkouts: Array<MeshCheckout>,
		name: (checkout: MeshCheckout) => string,
	) => {
		const keyOf = (checkout: MeshCheckout) => checkoutKey(checkout.machine, checkout.projectId);
		const shown = inOrder(checkouts, keyOf);
		shown.forEach((checkout, index) => {
			const key = keyOf(checkout);
			// A repo with no checkout here has nothing to load, so nothing to unfold.
			const folded = checkout.remoteOnly ? undefined : isFolded(key);
			rows.push({
				_tag: "Checkout",
				key,
				parentKey,
				depth: 1,
				positionInSet: index + 1,
				setSize: shown.length,
				folded,
				checkout,
				name: name(checkout),
			});
			if (folded === false) pushContents(checkout, key, 2);
		});
	};

	if (grouping === "machines") {
		inOrder(machines, (machine) => `machine:${machine.name}`).forEach((machine, index) => {
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
		const shown = inOrder(repos, (repo) => repoKey(repo.projectId));
		shown.forEach((repo, index) => {
			const key = repoKey(repo.projectId);
			const folded = isFolded(key);
			rows.push({
				_tag: "Repo",
				key,
				parentKey: null,
				depth: 0,
				positionInSet: index + 1,
				setSize: shown.length,
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

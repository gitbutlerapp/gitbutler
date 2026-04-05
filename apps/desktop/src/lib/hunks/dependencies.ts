import type { DiffHunk, HunkAssignment } from "$lib/hunks/hunk";

export type DependencyError = {
	description: string;
};

export function shouldRaiseDependencyError(
	error: DependencyError | null,
): error is DependencyError {
	if (!error) return false;
	if (error.description === "errors.projects.default_target.not_found") return false;
	return true;
}

export type CalculationError = {
	/**
	 * The error message.
	 */
	errorMessage: string;
	/**
	 * The stack ID that this error is associated with.
	 */
	stackId: string;
	/**
	 * The commit ID that this error is associated with.
	 */
	commitId: string;
	/**
	 * The file path that this error is associated with.
	 */
	path: string;
};

export type HunkLockTarget =
	| {
			type: "stack";
			subject: string;
	  }
	| {
			type: "unidentified";
	  };

/**
 * Represents the dependency of a diff hunk on a stack and commit.
 */
export type HunkLock = {
	/**
	 * The stack that this hunk is dependent on.
	 */
	target: HunkLockTarget;
	/**
	 * The commit ID that this hunk is dependent on.
	 */
	commitId: string;
};

/**
 * The map of file paths and hunks to their locks
 */
export type DiffDependency = [string, DiffHunk, HunkLock[]];

export type HunkDependencies = {
	/**
	 * The dependencies of the hunks in the diff.
	 */
	diffs: DiffDependency[];
	/**
	 * Errors that occurred while calculating dependencies.
	 */
	errors: CalculationError[];
};

export type HunkLocks = {
	/**
	 * The hunk in the change diff.
	 */
	hunk: DiffHunk;
	/**
	 * The dependencies of the hunk.
	 */
	locks: HunkLock[];
};

export type FileDependencies = {
	/**
	 * The file path of the diff.
	 */
	path: string;

	/**
	 * The dependencies of the diff.
	 */
	dependencies: HunkLocks[];
};
/**
 * Check whether two line ranges overlap.
 * Ranges are `[start, start + lines)` (1-based start, length in lines).
 * A range with 0 lines (pure insertion/deletion) is treated as a point at `start`.
 */
function rangesOverlap(startA: number, linesA: number, startB: number, linesB: number): boolean {
	const endA = startA + Math.max(linesA, 1);
	const endB = startB + Math.max(linesB, 1);
	return startA < endB && startB < endA;
}

/**
 * Filters dependency entries to only those whose hunk ranges overlap with
 * assignments visible in the current view.
 *
 * When `stackId` is provided, includes only dependencies overlapping assignments
 * assigned to that stack (matching the stack lane's visible hunks).
 * When `stackId` is undefined, includes only dependencies overlapping
 * unassigned assignments (matching the unassigned lane).
 */
export function filterDependenciesByAssignments(
	dependencies: HunkDependencies,
	assignments: HunkAssignment[],
	stackId: string | undefined,
): HunkDependencies {
	const filtered = dependencies.diffs.filter(([depPath, depHunk]) => {
		return assignments.some(
			(a) =>
				a.path === depPath &&
				a.hunkHeader !== null &&
				a.stackId === (stackId ?? null) &&
				rangesOverlap(
					depHunk.newStart,
					depHunk.newLines,
					a.hunkHeader.newStart,
					a.hunkHeader.newLines,
				),
		);
	});
	return { diffs: filtered, errors: dependencies.errors };
}

/**
 * Aggregates file dependencies from a collection of hunk dependencies.
 *
 * This function processes an array of `DiffDependency` objects and groups them
 * by file path, creating a list of `FileDependencies` where each file path
 * contains its associated hunk and lock dependencies. Additionally, it returns
 * a list of all unique file paths encountered during the aggregation process.
 *
 * @param hunkDependencies - An object containing the diffs to process.
 * @returns A tuple where:
 *          - The first element is an array of unique file paths.
 *          - The second element is an array of `FileDependencies` where each
 *            entry represents a file and its associated hunk and lock dependencies.
 *
 * @example
 * const hunkDependencies = {
 *   diffs: [
 *     ['file1.ts', 'hunk1', ['lock1']],
 *     ['file2.ts', 'hunk2', ['lock2']],
 *     ['file1.ts', 'hunk3', ['lock3']]
 *   ]
 * };
 * const result = aggregateFileDependencies(hunkDependencies);
 * // result:
 * // [
 * //   ['file1.ts', 'file2.ts'],
 * //   [
 * //     {
 * //       path: 'file1.ts',
 * //       dependencies: [
 * //         { hunk: 'hunk1', locks: ['lock1'] },
 * //         { hunk: 'hunk3', locks: ['lock3'] }
 * //       ]
 * //     },
 * //     {
 * //       path: 'file2.ts',
 * //       dependencies: [
 * //         { hunk: 'hunk2', locks: ['lock2'] }
 * //       ]
 * //     }
 * //   ]
 * // ]
 */
export function aggregateFileDependencies(
	hunkDependencies: HunkDependencies,
): [string[], FileDependencies[]] {
	const filePaths: string[] = [];
	const fileDependencies = hunkDependencies.diffs.reduce<FileDependencies[]>(
		(acc: FileDependencies[], diff: DiffDependency) => {
			const [path, hunk, locks] = diff;
			const exisitingDependency = acc.find((dep) => dep.path === path);
			if (exisitingDependency) {
				exisitingDependency.dependencies.push({
					hunk,
					locks,
				});
				return acc;
			}

			filePaths.push(path);

			return [
				...acc,
				{
					path,
					dependencies: [
						{
							hunk,
							locks,
						},
					],
				},
			];
		},
		[],
	);

	return [filePaths, fileDependencies];
}

/**
 * Checks if a file has any locked dependencies.
 *
 * @param filePath - The path of the file to check.
 * @param fileDependencies - Array of file dependencies to search through.
 * @returns `true` if the file has any locks, `false` otherwise.
 */
export function isFileLocked(filePath: string, fileDependencies: FileDependencies[]): boolean {
	const deps = fileDependencies.find((dep) => dep.path === filePath);
	return deps ? deps.dependencies.some((dep) => dep.locks.length > 0) : false;
}

/**
 * Extracts unique commit IDs from a file's dependencies.
 *
 * @param filePath - The path of the file to get commit IDs for.
 * @param fileDependencies - Array of file dependencies to search through.
 * @returns Array of unique commit IDs that the file depends on.
 */
export function getLockedCommitIds(
	filePath: string,
	fileDependencies: FileDependencies[],
): string[] {
	const deps = fileDependencies.find((dep) => dep.path === filePath);
	if (!deps) return [];

	const commitIds = new Set<string>();
	deps.dependencies.forEach((dep) => {
		dep.locks.forEach((lock) => commitIds.add(lock.commitId));
	});
	return Array.from(commitIds);
}

/**
 * Extracts unique stack IDs from a file's dependencies.
 *
 * @param filePath - The path of the file to get stack IDs for.
 * @param fileDependencies - Array of file dependencies to search through.
 * @returns Array of unique stack IDs that the file depends on.
 */
export function getLockedTargets(
	filePath: string,
	fileDependencies: FileDependencies[],
): HunkLockTarget[] {
	const deps = fileDependencies.find((dep) => dep.path === filePath);
	if (!deps) return [];

	const targets: HunkLockTarget[] = [];
	deps.dependencies.forEach((dep) => {
		dep.locks.forEach((lock) => {
			if (!targets.some((t) => targetEqual(t, lock.target))) {
				targets.push(lock.target);
			}
		});
	});
	return targets;
}

export function targetEqual(a: HunkLockTarget, b: HunkLockTarget) {
	if (a.type === "stack" && a.type === b.type) {
		return a.subject === b.subject;
	} else if (a.type === "unidentified" && a.type === b.type) {
		return true;
	} else {
		return false;
	}
}

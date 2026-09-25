import { useOpenInProgram } from "#ui/api/mutations.ts";
import { listProjectsQueryOptions, worktreesListQueryOptions } from "#ui/api/queries.ts";
import type { WorktreeListing } from "@gitbutler/but-sdk";
import { useQueryClient } from "@tanstack/react-query";

/**
 * The actions a checkout-relative path offers wherever it is shown — file rows,
 * hunks, the diff: turn it absolute, reveal it, open it. Each takes the linked
 * worktree the path lives in, or resolves against the project's own checkout
 * when that is unset.
 *
 * The listings that locate a checkout are read when the action runs rather than
 * subscribed to: nothing here renders them, and a surface that suspended on
 * them would tear down the very rows it lists.
 */

/** Where a linked worktree is checked out, whether it is active or archived. */
export const worktreePathIn = (listing: WorktreeListing, worktree: string): string | undefined =>
	[...listing.active, ...listing.archived].find((entry) => entry.name === worktree)?.path;

/** Turns a checkout-relative path absolute. */
export const useAbsolutePath = (
	projectId: string,
): ((path: string, worktree?: string) => Promise<string>) => {
	const queryClient = useQueryClient();

	return async (path, worktree) => {
		const basePath =
			worktree === undefined
				? (await queryClient.ensureQueryData(listProjectsQueryOptions)).find(
						(project) => project.id === projectId,
					)?.path
				: worktreePathIn(
						await queryClient.ensureQueryData(worktreesListQueryOptions(projectId)),
						worktree,
					);
		if (basePath === undefined) throw new Error(`Could not find checkout for ${path}`);

		return window.lite.pathJoin(basePath, path);
	};
};

/** Reveals a checkout-relative path in the OS file manager. */
export const useRevealInFolder = (
	projectId: string,
): ((path: string, worktree?: string) => Promise<void>) => {
	const absolutePath = useAbsolutePath(projectId);

	return async (path, worktree) => {
		await window.lite.showItemInFolder(await absolutePath(path, worktree));
	};
};

/**
 * Opens a checkout-relative path in a program. The backend resolves the path
 * against the project's checkout, so a linked worktree's file is handed over
 * absolute, which joining leaves untouched.
 */
export const useOpenPathInProgram = (
	projectId: string,
): {
	isPending: boolean;
	openPathInProgram: (args: {
		programId: string;
		path: string;
		lineNr: number | null;
		worktree?: string;
	}) => Promise<void>;
} => {
	const absolutePath = useAbsolutePath(projectId);
	const { isPending, mutate: openInProgram } = useOpenInProgram();

	return {
		isPending,
		openPathInProgram: async ({ programId, path, lineNr, worktree }) => {
			const target = worktree === undefined ? path : await absolutePath(path, worktree);
			openInProgram({ projectId, programId, path: target, lineNr });
		},
	};
};

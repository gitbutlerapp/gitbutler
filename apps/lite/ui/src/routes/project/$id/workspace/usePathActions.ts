import { useOpenInProgram } from "#ui/api/mutations.ts";
import { changesSourceOf } from "#ui/addresses.ts";

/**
 * The actions a checkout-relative path offers wherever it is shown — file rows,
 * hunks, the diff: turn it absolute, reveal it, open it. Each takes the linked
 * worktree the path lives in, or the project's own checkout when that is unset,
 * and the backend resolves the path against that checkout.
 */

/** Turns a checkout-relative path absolute. */
export const useAbsolutePath =
	(projectId: string): ((path: string, worktree?: string) => Promise<string>) =>
	(path, worktree) =>
		window.lite.workdirPath({ projectId, checkout: changesSourceOf({ worktree }), path });

/** Reveals a checkout-relative path in the OS file manager. */
export const useRevealInFolder =
	(projectId: string): ((path: string, worktree?: string) => Promise<void>) =>
	(path, worktree) =>
		window.lite.showInFinder({ projectId, checkout: changesSourceOf({ worktree }), path });

/** Opens a checkout-relative path in a program. */
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
	const { isPending, mutate: openInProgram } = useOpenInProgram();

	return {
		isPending,
		openPathInProgram: async ({ programId, path, lineNr, worktree }) => {
			openInProgram({
				projectId,
				programId,
				checkout: changesSourceOf({ worktree }),
				path,
				lineNr,
			});
		},
	};
};

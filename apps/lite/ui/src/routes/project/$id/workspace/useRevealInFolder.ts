import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { useSuspenseQuery } from "@tanstack/react-query";

/**
 * Reveals a repository-relative path in the OS file manager. Only the
 * project's location turns that path absolute, so it is resolved here rather
 * than at each of the surfaces — file rows, hunks, the diff — that offer the
 * action. Selecting the path alone keeps callers off the projects list's
 * identity.
 */
export const useRevealInFolder = (projectId: string): ((path: string) => Promise<void>) => {
	const { data: projectPath } = useSuspenseQuery({
		...listProjectsQueryOptions,
		select: (projects) => projects.find((project) => project.id === projectId)?.path,
	});
	if (projectPath === undefined) throw new Error("Could not find selected project");

	return async (path) => {
		const absolutePath = await window.lite.pathJoin(projectPath, path);
		await window.lite.showItemInFolder(absolutePath);
	};
};

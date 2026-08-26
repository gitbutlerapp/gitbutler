import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { useQueryClient } from "@tanstack/react-query";

/**
 * Reveals a repository-relative path in the OS file manager. Only the
 * project's location turns that path absolute, so it is resolved here rather
 * than at each of the surfaces — file rows, hunks, the diff — that offer the
 * action. The list is read when the action runs rather than subscribed to:
 * nothing here renders it, and a surface that suspended on it would tear down
 * the very rows it lists.
 */
export const useRevealInFolder = (projectId: string): ((path: string) => Promise<void>) => {
	const queryClient = useQueryClient();

	return async (path) => {
		const projects = queryClient.getQueryData(listProjectsQueryOptions.queryKey);
		const projectPath = projects?.find((project) => project.id === projectId)?.path;
		if (projectPath === undefined) throw new Error("Could not find selected project");

		const absolutePath = await window.lite.pathJoin(projectPath, path);
		await window.lite.showItemInFolder(absolutePath);
	};
};

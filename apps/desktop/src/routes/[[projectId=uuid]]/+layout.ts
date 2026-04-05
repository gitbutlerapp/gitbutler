import { error } from "@sveltejs/kit";
import type { LayoutLoad } from "./$types";

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params, parent }) => {
	const { pinnedProjectId } = await parent();
	const projectId = params.projectId ?? pinnedProjectId ?? undefined;
	if (!projectId) {
		error(404, "No project");
	}
	return {
		projectId,
		projectPinned: !params.projectId,
	};
};

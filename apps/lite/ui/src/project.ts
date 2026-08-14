const lastOpenedProjectKey = "lastProject";
const lastPlaceKey = "lastPlace";

export const readLastOpenedProject = (): string | null =>
	window.localStorage.getItem(lastOpenedProjectKey);

export const writeLastOpenedProject = (projectId: string): void =>
	window.localStorage.setItem(lastOpenedProjectKey, projectId);

/**
 * Where the user was, so a relaunch reopens it rather than just the project.
 * Stored with its project so a stale search never lands on another one.
 */
export const readLastPlace = (): { projectId: string; search: string } | null => {
	const raw = window.localStorage.getItem(lastPlaceKey);
	if (raw === null) return null;

	try {
		const parsed: unknown = JSON.parse(raw);
		return typeof parsed === "object" &&
			parsed !== null &&
			typeof (parsed as { projectId?: unknown }).projectId === "string" &&
			typeof (parsed as { search?: unknown }).search === "string"
			? (parsed as { projectId: string; search: string })
			: null;
	} catch {
		return null;
	}
};

export const writeLastPlace = (projectId: string, search: string): void =>
	window.localStorage.setItem(lastPlaceKey, JSON.stringify({ projectId, search }));

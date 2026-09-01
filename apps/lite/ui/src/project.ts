const lastOpenedProjectKey = "lastProject";
const lastPlaceKey = "lastPlace";
const openedAtKey = "projectOpenedAt";

export const readLastOpenedProject = (): string | null =>
	window.localStorage.getItem(lastOpenedProjectKey);

/**
 * When each project was last opened, so a picker can lead with the ones in use. Keyed by project
 * id; a project absent from it has not been opened since this was first recorded.
 */
export const readProjectsOpenedAt = (): Record<string, number> => {
	const raw = window.localStorage.getItem(openedAtKey);
	if (raw === null) return {};

	try {
		const parsed: unknown = JSON.parse(raw);
		if (typeof parsed !== "object" || parsed === null) return {};
		return Object.fromEntries(
			Object.entries(parsed as Record<string, unknown>).filter(
				([, at]) => typeof at === "number" && Number.isFinite(at),
			) as Array<[string, number]>,
		);
	} catch {
		return {};
	}
};

/**
 * Records the project as the one to reopen, and stamps when it was opened. The two go together:
 * every way into a project passes through here, so nothing else has to remember to keep the
 * history in step.
 */
export const writeLastOpenedProject = (projectId: string): void => {
	window.localStorage.setItem(lastOpenedProjectKey, projectId);
	window.localStorage.setItem(
		openedAtKey,
		JSON.stringify({ ...readProjectsOpenedAt(), [projectId]: Date.now() }),
	);
};

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

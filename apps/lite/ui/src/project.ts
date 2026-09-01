const lastOpenedProjectKey = "lastProject";
const lastPlaceKey = "lastPlace";
const openedAtKey = "projectOpenedAt";
const repoMarksKey = "projectRepoMarks";

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

/** What the forge says about a project's repository, as last seen. */
export type ProjectRepoMarks = {
	/** `null` where the forge does not report it — Azure, or a token without the scope. */
	private: boolean | null;
	fork: boolean;
};

/**
 * What each project's repository is, as last seen on its forge.
 *
 * None of it is knowable from a clone — a private repo and a public one are identical on disk —
 * so it has to be asked of the forge and remembered. A project absent from this has not been open
 * since the record began, or has no forge to ask.
 */
export const readProjectsRepoMarks = (): Record<string, ProjectRepoMarks> => {
	const raw = window.localStorage.getItem(repoMarksKey);
	if (raw === null) return {};

	try {
		const parsed: unknown = JSON.parse(raw);
		if (typeof parsed !== "object" || parsed === null) return {};
		return Object.fromEntries(
			Object.entries(parsed as Record<string, unknown>).filter(([, marks]) => {
				if (typeof marks !== "object" || marks === null) return false;
				const { private: isPrivate, fork } = marks as Partial<ProjectRepoMarks>;
				return (typeof isPrivate === "boolean" || isPrivate === null) && typeof fork === "boolean";
			}) as Array<[string, ProjectRepoMarks]>,
		);
	} catch {
		return {};
	}
};

export const writeProjectRepoMarks = (projectId: string, marks: ProjectRepoMarks): void => {
	window.localStorage.setItem(
		repoMarksKey,
		JSON.stringify({ ...readProjectsRepoMarks(), [projectId]: marks }),
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

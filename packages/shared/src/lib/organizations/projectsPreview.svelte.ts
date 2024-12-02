import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { projectsSelectors } from '$lib/organizations/projectsSlice';
import type { ProjectService } from '$lib/organizations/projectService';
import type { Project } from '$lib/organizations/types';
import type { AppOrganizationsState, AppProjectsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function getParentForRepositoryId(
	appState: AppProjectsState & AppOrganizationsState,
	projectService: ProjectService,
	projectRepositoryId: string
): Reactive<Project | undefined> {
	const current = $derived.by(() => {
		registerInterest(projectService.getProjectInterest(projectRepositoryId));
		const project = projectsSelectors.selectById(appState.projects, projectRepositoryId);

		if (!project || !project.parentProjectRepositoryId) return;

		registerInterest(projectService.getProjectInterest(project.parentProjectRepositoryId));
		return projectsSelectors.selectById(appState.projects, project.parentProjectRepositoryId);
	});

	return {
		get current() {
			return current;
		}
	};
}

export function getFeedIdentityForRepositoryId(
	appState: AppProjectsState & AppOrganizationsState,
	projectService: ProjectService,
	projectRepositoryId: string
): Reactive<string | undefined> {
	const parentProject = $derived(
		getParentForRepositoryId(appState, projectService, projectRepositoryId)
	);

	const current = $derived.by(() => {
		if (!parentProject.current) return;

		return `${parentProject.current.owner}/${parentProject.current.slug}`;
	});

	return {
		get current() {
			return current;
		}
	};
}

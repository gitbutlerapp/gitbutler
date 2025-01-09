import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import { projectsSelectors } from '$lib/organizations/projectsSlice';
import type { Loadable } from '$lib/network/types';
import type { ProjectService } from '$lib/organizations/projectService';
import type { LoadableProject } from '$lib/organizations/types';
import type { AppOrganizationsState, AppProjectsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function getProjectByRepositoryId(
	appState: AppProjectsState,
	projectService: ProjectService,
	projectRepositoryId: string
): Reactive<LoadableProject | undefined> {
	registerInterest(projectService.getProjectInterest(projectRepositoryId));
	const current = $derived(projectsSelectors.selectById(appState.projects, projectRepositoryId));

	return {
		get current() {
			return current;
		}
	};
}

export function getParentForRepositoryId(
	appState: AppProjectsState & AppOrganizationsState,
	projectService: ProjectService,
	projectRepositoryId: string
): Reactive<LoadableProject | undefined> {
	const current = $derived.by(() => {
		const project = getProjectByRepositoryId(appState, projectService, projectRepositoryId);

		if (!isFound(project.current) || !project.current.value.parentProjectRepositoryId) return;

		return getProjectByRepositoryId(
			appState,
			projectService,
			project.current.value.parentProjectRepositoryId
		);
	});

	return {
		get current() {
			return current?.current;
		}
	};
}

export function getFeedIdentityForRepositoryId(
	appState: AppProjectsState & AppOrganizationsState,
	projectService: ProjectService,
	projectRepositoryId: string
): Reactive<Loadable<string>> {
	const parentProject = $derived(
		getParentForRepositoryId(appState, projectService, projectRepositoryId)
	);

	const current = $derived.by<Loadable<string>>(() => {
		if (!isFound(parentProject.current)) return parentProject.current || { status: 'loading' };

		return {
			status: 'found',
			value: `${parentProject.current.value.owner}/${parentProject.current.value.slug}`
		};
	});

	return {
		get current() {
			return current;
		}
	};
}

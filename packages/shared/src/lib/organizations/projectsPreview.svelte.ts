import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { projectsSelectors } from '$lib/organizations/projectsSlice';
import type { Loadable } from '$lib/network/loadable';
import type { ProjectService } from '$lib/organizations/projectService';
import type { LoadableProject } from '$lib/organizations/types';
import type { AppOrganizationsState, AppProjectsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function getParentForRepositoryId(
	appState: AppProjectsState & AppOrganizationsState,
	projectService: ProjectService,
	projectRepositoryId: string
): Reactive<LoadableProject | undefined> {
	registerInterest(projectService.getProjectInterest(projectRepositoryId));

	const current = $derived.by(() => {
		const loadableProject = projectsSelectors.selectById(appState.projects, projectRepositoryId);

		if (
			!loadableProject ||
			loadableProject.type !== 'found' ||
			!loadableProject.value.parentProjectRepositoryId
		)
			return;

		registerInterest(
			projectService.getProjectInterest(loadableProject.value.parentProjectRepositoryId)
		);

		return projectsSelectors.selectById(
			appState.projects,
			loadableProject.value.parentProjectRepositoryId
		);
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
): Reactive<Loadable<string>> {
	const parentProject = $derived(
		getParentForRepositoryId(appState, projectService, projectRepositoryId)
	);

	const current = $derived.by<Loadable<string>>(() => {
		if (!parentProject.current) return { type: 'loading' };
		if (parentProject.current.type !== 'found') return parentProject.current;

		return {
			type: 'found',
			value: `${parentProject.current.value.owner}/${parentProject.current.value.slug}`
		};
	});

	return {
		get current() {
			return current;
		}
	};
}

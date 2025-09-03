import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { isFound, map } from '$lib/network/loadable';
import { PROJECT_SERVICE } from '$lib/organizations/projectService';
import { projectTable } from '$lib/organizations/projectsSlice';
import { lookupProject } from '$lib/organizations/repositoryIdLookupPreview.svelte';
import { reactive } from '$lib/reactiveUtils.svelte';
import { APP_STATE } from '$lib/redux/store.svelte';
import { type Reactive } from '$lib/storeUtils';
import { inject } from '@gitbutler/core/context';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { Loadable } from '$lib/network/types';
import type { LoadableProject } from '$lib/organizations/types';

export function getProject(
	ownerSlug: string,
	projectSlug: string,
	inView?: InView
): Reactive<LoadableProject | undefined> {
	const repositoryId = lookupProject(ownerSlug, projectSlug, inView);
	const current = $derived(
		map(repositoryId.current, (repositoryId) => getProjectByRepositoryId(repositoryId, inView))
	);

	return reactive(() => current?.current);
}

export function getProjectByRepositoryId(
	projectRepositoryId: string,
	inView?: InView
): Reactive<LoadableProject | undefined> {
	const appState = inject(APP_STATE);
	const projectService = inject(PROJECT_SERVICE);
	registerInterest(projectService.getProjectInterest(projectRepositoryId), inView);
	const current = $derived(
		projectTable.selectors.selectById(appState.projects, projectRepositoryId)
	);

	return reactive(() => current);
}

export function getAllUserProjects(user: string, inView?: InView): Reactive<LoadableProject[]> {
	const appState = inject(APP_STATE);
	const projectService = inject(PROJECT_SERVICE);
	registerInterest(projectService.getAllProjectsInterest(), inView);
	const current = $derived.by(() => {
		const allProjects = projectTable.selectors.selectAll(appState.projects);
		return allProjects.filter((project) => isFound(project) && project.value.owner === user);
	});

	return reactive(() => current);
}

export function getRecentlyInteractedProjects(inView?: InView): Reactive<LoadableProject[]> {
	const appState = inject(APP_STATE);
	const projectService = inject(PROJECT_SERVICE);
	registerInterest(projectService.getRecentProjectsInterest(), inView);
	const current = $derived(
		appState.recentlyInteractedProjectIds.recentlyInteractedProjectIds
			.map((recentProjectId) =>
				projectTable.selectors.selectById(appState.projects, recentProjectId)
			)
			.filter(isDefined)
	);

	return reactive(() => current);
}

export function getRecentlyPushedProjects(inView?: InView): Reactive<LoadableProject[]> {
	const appState = inject(APP_STATE);
	const projectService = inject(PROJECT_SERVICE);
	registerInterest(projectService.getRecentlyPushedProjectsInterest(), inView);
	const current = $derived(
		appState.recentlyPushedProjectIds.recentlyPushedProjectIds
			.map((recentProjectId) =>
				projectTable.selectors.selectById(appState.projects, recentProjectId)
			)
			.filter(isDefined)
	);

	return reactive(() => current);
}

export function getParentForRepositoryId(
	projectRepositoryId: string,
	inView?: InView
): Reactive<LoadableProject | undefined> {
	const current = $derived.by(() => {
		const project = getProjectByRepositoryId(projectRepositoryId, inView);

		if (!isFound(project.current) || !project.current.value.parentProjectRepositoryId) return;

		return getProjectByRepositoryId(project.current.value.parentProjectRepositoryId, inView);
	});

	return reactive(() => current?.current);
}

export function getFeedIdentityForRepositoryId(
	projectRepositoryId: string,
	inView?: InView
): Reactive<Loadable<string>> {
	const parentProject = $derived(getParentForRepositoryId(projectRepositoryId, inView));

	const current = $derived.by<Loadable<string>>(() => {
		if (!isFound(parentProject.current)) return parentProject.current || { status: 'loading' };

		return {
			status: 'found',
			value: `${parentProject.current.value.owner}/${parentProject.current.value.slug}`
		};
	});

	return reactive(() => current);
}

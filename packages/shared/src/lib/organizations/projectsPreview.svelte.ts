import { getContext } from '$lib/context';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import { ProjectService } from '$lib/organizations/projectService';
import { projectTable } from '$lib/organizations/projectsSlice';
import { AppState } from '$lib/redux/store.svelte';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { Loadable } from '$lib/network/types';
import type { LoadableProject } from '$lib/organizations/types';
import type { Reactive } from '$lib/storeUtils';

export function getProjectByRepositoryId(
	projectRepositoryId: string,
	inView?: InView
): Reactive<LoadableProject | undefined> {
	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	registerInterest(projectService.getProjectInterest(projectRepositoryId), inView);
	const current = $derived(
		projectTable.selectors.selectById(appState.projects, projectRepositoryId)
	);

	return {
		get current() {
			return current;
		}
	};
}

export function getAllUserProjects(user: string, inView?: InView): Reactive<LoadableProject[]> {
	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	registerInterest(projectService.getAllProjectsInterest(), inView);
	const current = $derived.by(() => {
		const allProjects = projectTable.selectors.selectAll(appState.projects);
		return allProjects.filter((project) => isFound(project) && project.value.owner === user);
	});

	return {
		get current() {
			return current;
		}
	};
}

export function getRecentlyInteractedProjects(inView?: InView): Reactive<LoadableProject[]> {
	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	registerInterest(projectService.getRecentProjectsInterest(), inView);
	const current = $derived(
		appState.recentlyInteractedProjectIds.recentlyInteractedProjectIds
			.map((recentProjectId) =>
				projectTable.selectors.selectById(appState.projects, recentProjectId)
			)
			.filter(isDefined)
	);

	return {
		get current() {
			return current;
		}
	};
}

export function getRecentlyPushedProjects(inView?: InView): Reactive<LoadableProject[]> {
	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	registerInterest(projectService.getRecentlyPushedProjectsInterest(), inView);
	const current = $derived(
		appState.recentlyPushedProjectIds.recentlyPushedProjectIds
			.map((recentProjectId) =>
				projectTable.selectors.selectById(appState.projects, recentProjectId)
			)
			.filter(isDefined)
	);

	return {
		get current() {
			return current;
		}
	};
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

	return {
		get current() {
			return current?.current;
		}
	};
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

	return {
		get current() {
			return current;
		}
	};
}

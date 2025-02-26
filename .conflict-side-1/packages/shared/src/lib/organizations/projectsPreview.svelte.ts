import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import {
	getOrganizationProjects,
	getOrganizations
} from '$lib/organizations/organizationsPreview.svelte';
import { projectsSelectors } from '$lib/organizations/projectsSlice';
import type { Loadable } from '$lib/network/types';
import type { OrganizationService } from '$lib/organizations/organizationService';
import type { ProjectService } from '$lib/organizations/projectService';
import type { LoadableOrganization, LoadableProject } from '$lib/organizations/types';
import type { AppOrganizationsState, AppProjectsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function getProjectByRepositoryId(
	appState: AppProjectsState,
	projectService: ProjectService,
	projectRepositoryId: string,
	inView?: InView
): Reactive<LoadableProject | undefined> {
	registerInterest(projectService.getProjectInterest(projectRepositoryId), inView);
	const current = $derived(projectsSelectors.selectById(appState.projects, projectRepositoryId));

	return {
		get current() {
			return current;
		}
	};
}

export function getAllUserProjects(
	user: string,
	appState: AppProjectsState,
	projectService: ProjectService,
	inView?: InView
): Reactive<LoadableProject[]> {
	registerInterest(projectService.getAllProjectsInterest(), inView);
	const current = $derived.by(() => {
		const allProjects = projectsSelectors.selectAll(appState.projects);
		return allProjects.filter((project) => isFound(project) && project.value.owner === user);
	});

	return {
		get current() {
			return current;
		}
	};
}

export function getAllUserRelatedProjects(
	appState: AppProjectsState & AppOrganizationsState,
	projectService: ProjectService,
	organizationService: OrganizationService,
	user: string,
	inView?: InView
): Reactive<LoadableProject[]> {
	registerInterest(projectService.getAllProjectsInterest(), inView);
	const userProjects = $derived.by(() => {
		const allProjects = projectsSelectors.selectAll(appState.projects);
		return allProjects.filter(
			(project) => isFound(project) && project.value.owner === user
		) as LoadableProject[];
	});

	const organizations = $derived(getOrganizations(appState, organizationService));
	const reactiveOrganizationProjects = $derived.by(() => {
		if (!organizations.current) return [];

		const foundOrganizations = organizations.current.filter((organization) =>
			isFound(organization)
		) as (LoadableOrganization & { status: 'found' })[];

		return foundOrganizations.flatMap((organization) => {
			return getOrganizationProjects(
				appState,
				organizationService,
				organization.value.slug,
				inView
			);
		});
	});
	const organizationProjects = $derived(
		reactiveOrganizationProjects
			.map((a) => a.current)
			.filter((a) => a !== undefined)
			.flat() as LoadableProject[]
	);

	return {
		get current() {
			return [...userProjects, ...organizationProjects];
		}
	};
}

export function getParentForRepositoryId(
	appState: AppProjectsState & AppOrganizationsState,
	projectService: ProjectService,
	projectRepositoryId: string,
	inView?: InView
): Reactive<LoadableProject | undefined> {
	const current = $derived.by(() => {
		const project = getProjectByRepositoryId(appState, projectService, projectRepositoryId, inView);

		if (!isFound(project.current) || !project.current.value.parentProjectRepositoryId) return;

		return getProjectByRepositoryId(
			appState,
			projectService,
			project.current.value.parentProjectRepositoryId,
			inView
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
	projectRepositoryId: string,
	inView?: InView
): Reactive<Loadable<string>> {
	const parentProject = $derived(
		getParentForRepositoryId(appState, projectService, projectRepositoryId, inView)
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

import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { ProjectService, ProjectsService } from '$lib/project/projects';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';
import type { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
import type { AppProjectsState } from '@gitbutler/shared/redux/store.svelte';

export function projectCloudSync(
	appState: AppProjectsState,
	projectsService: ProjectsService,
	projectService: ProjectService,
	cloudProjectService: CloudProjectService,
	httpClient: HttpClient
) {
	const project = readableToReactive(projectService.project);
	const authentictionAvailable = readableToReactive(httpClient.authenticationAvailable);

	const loadableCloudProject = $derived(
		project.current?.api && authentictionAvailable
			? getProjectByRepositoryId(appState, cloudProjectService, project.current.api.repository_id)
			: undefined
	);

	$effect(() => {
		if (
			!project.current?.api ||
			!loadableCloudProject?.current ||
			loadableCloudProject?.current.status !== 'found'
		)
			return;

		const cloudProject = loadableCloudProject.current.value;
		const persistedProjectUpdatedAt = new Date(project.current.api.updated_at).getTime();
		const cloudProjectUpdatedAt = new Date(cloudProject.updatedAt).getTime();
		if (persistedProjectUpdatedAt >= cloudProjectUpdatedAt) return;

		const mutableProject = structuredClone(project.current);
		mutableProject.api = {
			name: cloudProject.name,
			description: cloudProject.description,
			repository_id: cloudProject.repositoryId,
			git_url: cloudProject.gitUrl,
			git_code_url: cloudProject.codeGitUrl,
			created_at: cloudProject.createdAt,
			updated_at: cloudProject.updatedAt,
			sync: mutableProject.api?.sync ?? false,
			sync_code: mutableProject.api?.sync_code ?? false
		};

		projectsService.updateProject(mutableProject);
	});
}

import { isFound, isNotFound } from '@gitbutler/shared/network/loadable';
import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { shallowCompare } from '@gitbutler/shared/shallowCompare';
import type { Project } from '$lib/project/project';
import type { ProjectService } from '$lib/project/projectService';
import type { ProjectsService } from '$lib/project/projectsService';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

export function projectCloudSync(
	projectsService: ProjectsService,
	projectService: ProjectService,
	httpClient: HttpClient
) {
	const project = readableToReactive(projectService.project);
	const authentictionAvailable = readableToReactive(httpClient.authenticationAvailable);

	const loadableCloudProject = $derived(
		project.current?.api && authentictionAvailable
			? getProjectByRepositoryId(project.current.api.repository_id)
			: undefined
	);

	$effect(() => {
		if (!project.current?.api || !isFound(loadableCloudProject?.current)) {
			// If the project is 404 from the server, but recorded on the
			// client, assume it has been deleted on the server and we should
			// clean it up.
			if (isNotFound(loadableCloudProject?.current) && project.current?.api) {
				const mutableProject: Project & { unset_api?: boolean } = structuredClone(project.current);
				mutableProject.api = undefined;
				mutableProject.unset_api = true;
				projectsService.updateProject(mutableProject);
			}

			return;
		}

		const cloudProject = loadableCloudProject.current.value;
		const mutableProject = structuredClone(project.current);

		const newDetails = {
			name: cloudProject.name,
			description: cloudProject.description,
			repository_id: cloudProject.repositoryId,
			git_url: cloudProject.gitUrl,
			code_git_url: cloudProject.codeGitUrl,
			created_at: cloudProject.createdAt,
			updated_at: cloudProject.updatedAt,
			sync: mutableProject.api?.sync ?? false,
			sync_code: mutableProject.api?.sync_code ?? false,
			reviews: mutableProject.api?.reviews ?? false
		};

		if (shallowCompare(newDetails, mutableProject.api)) {
			return;
		}

		mutableProject.api = newDetails;

		projectsService.updateProject(mutableProject);
	});
}

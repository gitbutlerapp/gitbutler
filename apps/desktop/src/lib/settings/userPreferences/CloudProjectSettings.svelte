<script lang="ts">
	import { Project, ProjectsService } from '$lib/backend/projects';
	import Section from '$lib/settings/Section.svelte';
	import ProjectConnectModal from '$lib/settings/userPreferences/ProjectConnectModal.svelte';
	import Link from '$lib/shared/Link.svelte';
	import { User } from '$lib/stores/user';
	import * as toasts from '$lib/utils/toasts';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { organizationsSelectors } from '@gitbutler/shared/organizations/organizationsSlice';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { projectsSelectors } from '@gitbutler/shared/organizations/projectsSlice';
	import { AppState } from '@gitbutler/shared/redux/store';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	const appState = getContext(AppState);
	const _project = getContext(Project);
	const user = getContextStore(User);
	const projectsService = getContext(ProjectsService);
	const cloudProjectService = getContext(CloudProjectService);
	const organizationService = getContext(OrganizationService);

	const project = projectsService.getProjectStore(_project.id);

	const projects = appState.projects;
	const cloudProject = $derived(
		$project?.api ? projectsSelectors.selectById($projects, $project.api.repository_id) : undefined
	);
	const cloudProjectInterest = $derived(
		$project?.api ? cloudProjectService.getProjectInterest($project.api.repository_id) : undefined
	);

	const organizations = appState.organizations;
	const usersOrganizations = $derived(organizationsSelectors.selectAll($organizations));
	const usersOrganizationsInterest = organizationService.getOrganizationListingInterest();

	async function createProject() {
		if (!$project) return;

		const fetchedCloudProject = await cloudProjectService.createProject(
			$project.title,
			$project.description
		);

		const mutableProject = structuredClone($project);
		mutableProject.api = {
			name: fetchedCloudProject.name,
			description: fetchedCloudProject.description,
			repository_id: fetchedCloudProject.repositoryId,
			git_url: fetchedCloudProject.gitUrl,
			// code_git_url: fetchedCloudProject.codeGitUrl,
			created_at: fetchedCloudProject.createdAt,
			updated_at: fetchedCloudProject.updatedAt,
			sync: false,
			sync_code: false
		};
		await projectsService.updateProject(mutableProject);
	}

	async function onSyncChange(sync: boolean) {
		if (!$user) return;
		if (!$project?.api) return;
		try {
			const mutableProject = structuredClone($project);
			mutableProject!.api!.sync = sync;
			projectsService.updateProject(mutableProject);
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	}
	// These functions are disgusting
	async function onSyncCodeChange(sync_code: boolean) {
		if (!$user) return;
		if (!$project?.api) return;
		try {
			const mutableProject = structuredClone($project);
			mutableProject!.api!.sync_code = sync_code;
			projectsService.updateProject(mutableProject);
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	}
</script>

{#if cloudProjectInterest}
	<RegisterInterest interest={cloudProjectInterest} />
{/if}

{#if cloudProject}
	<Section>
		{#snippet title()}
			Full data synchronization
		{/snippet}

		{#snippet children()}
			<SectionCard labelFor="historySync" orientation="row">
				{#snippet caption()}
					Sync this project's operations log with GitButler Web services. The operations log
					includes snapshots of the repository state, including non-committed code changes.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="historySync"
						checked={$project?.api?.sync || false}
						onclick={async () => await onSyncChange(!$project?.api?.sync)}
					/>
				{/snippet}
			</SectionCard>
			<SectionCard labelFor="branchesySync" orientation="row">
				{#snippet caption()}
					Sync this repository's branches with the GitButler Remote.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="branchesySync"
						checked={$project?.api?.sync_code || false}
						onclick={async () => await onSyncCodeChange(!$project?.api?.sync_code)}
					/>
				{/snippet}
			</SectionCard>

			{#if $project?.api}
				<div class="api-link text-12">
					<Link
						target="_blank"
						rel="noreferrer"
						href="{PUBLIC_API_BASE_URL}projects/{$project.api?.repository_id}"
						>Go to GitButler Cloud Project</Link
					>
				</div>
			{/if}
		{/snippet}
	</Section>

	{#if !cloudProject?.parentProjectRepositoryId}
		<Section>
			{#snippet title()}
				Link your project with an organization
			{/snippet}

			<RegisterInterest interest={usersOrganizationsInterest} />

			<div>
				{#each usersOrganizations as organization, index}
					<SectionCard
						roundedBottom={index === usersOrganizations.length - 1}
						roundedTop={index === 0}
						orientation="row"
						centerAlign
					>
						{#snippet children()}
							<h5 class="text-15 text-bold flex-grow">{organization.name || organization.slug}</h5>
						{/snippet}
						{#snippet actions()}
							<ProjectConnectModal
								organizationSlug={organization.slug}
								projectRepositoryId={cloudProject.repositoryId}
							/>
						{/snippet}
					</SectionCard>
				{/each}
			</div>
		</Section>
	{/if}
{:else if !$project?.api?.repository_id}
	<Section>
		<Button onclick={createProject}>Create Gitbutler Project</Button>
	</Section>
{:else}
	<p>Loading...</p>
{/if}

<style>
	.api-link {
		display: flex;
		justify-content: flex-end;
	}

	.flex-grow {
		flex-grow: 1;
	}
</style>

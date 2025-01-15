<script lang="ts">
	import Link from '$components/Link.svelte';
	import ProjectConnectModal from '$components/ProjectConnectModal.svelte';
	import Section from '$components/Section.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { ProjectsService } from '$lib/project/projectsService';
	import { User } from '$lib/user/user';
	import * as toasts from '$lib/utils/toasts';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { organizationsSelectors } from '@gitbutler/shared/organizations/organizationsSlice';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { projectsSelectors } from '@gitbutler/shared/organizations/projectsSlice';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	const appState = getContext(AppState);
	const user = getContextStore(User);
	const projectsService = getContext(ProjectsService);
	const projectService = getContext(ProjectService);
	const cloudProjectService = getContext(CloudProjectService);
	const organizationService = getContext(OrganizationService);

	const project = projectService.project;

	const cloudProject = $derived(
		$project?.api
			? projectsSelectors.selectById(appState.projects, $project.api.repository_id)
			: undefined
	);
	const cloudProjectInterest = $derived(
		$project?.api ? cloudProjectService.getProjectInterest($project.api.repository_id) : undefined
	);

	const usersOrganizations = $derived(organizationsSelectors.selectAll(appState.organizations));
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
			git_code_url: fetchedCloudProject.codeGitUrl,
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

	<Loading loadable={cloudProject}>
		{#snippet children(cloudProject)}
			{#if !cloudProject.parentProjectRepositoryId}
				<Section>
					{#snippet title()}
						Link your project with an organization
					{/snippet}

					<RegisterInterest interest={usersOrganizationsInterest} />

					<div>
						{#each usersOrganizations as loadableOrganization, index}
							<SectionCard
								roundedBottom={index === usersOrganizations.length - 1}
								roundedTop={index === 0}
								orientation="row"
								centerAlign
							>
								{#snippet children()}
									<Loading loadable={loadableOrganization}>
										{#snippet children(organization)}
											<h5 class="text-15 text-bold flex-grow">
												{organization.name || organization.slug}
											</h5>
										{/snippet}
									</Loading>
								{/snippet}
								{#snippet actions()}
									<ProjectConnectModal
										organizationSlug={loadableOrganization.id}
										projectRepositoryId={cloudProject.repositoryId}
									/>
								{/snippet}
							</SectionCard>
						{/each}
					</div>
				</Section>
			{/if}
		{/snippet}
	</Loading>
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

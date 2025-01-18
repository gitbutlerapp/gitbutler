<script lang="ts">
	import Link from '$components/Link.svelte';
	import ProjectConnectModal from '$components/ProjectConnectModal.svelte';
	import Section from '$components/Section.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { ProjectsService } from '$lib/project/projectsService';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound, map } from '@gitbutler/shared/network/loadable';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizations } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import type { Project } from '@gitbutler/shared/organizations/types';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	const appState = getContext(AppState);
	const projectsService = getContext(ProjectsService);
	const projectService = getContext(ProjectService);
	const cloudProjectService = getContext(CloudProjectService);
	const organizationService = getContext(OrganizationService);
	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const userService = getContext(UserService);

	const project = projectService.project;
	const userLogin = userService.userLogin;

	const cloudProject = $derived(
		$project?.api?.repository_id
			? getProjectByRepositoryId(appState, cloudProjectService, $project.api.repository_id)
			: undefined
	);

	let organizationsList = $state<HTMLElement>();
	const usersOrganizations = $derived(
		getOrganizations(appState, organizationService, { element: organizationsList })
	);

	const existingProjectRepositoryId = $derived(
		$userLogin && $project?.title
			? lookupProject(appState, repositoryIdLookupService, $userLogin, $project.title)
			: undefined
	);
	const existingProject = $derived(
		map(existingProjectRepositoryId?.current, (repositoryId) =>
			getProjectByRepositoryId(appState, cloudProjectService, repositoryId)
		)
	);

	$effect(() => {
		// We want to make use of this value in the `createProject` function
		// and nowhere else, as such, by default svelte doesn't ever subscribe
		// to it. We just need to explicity tell svelte that we care about
		// this value, and we want it to be subscribed for the duration of the
		// component
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		existingProject;
	});

	async function createProject() {
		if (!$project) return;

		let fetchedCloudProject: Project;

		if (isFound(existingProject?.current)) {
			fetchedCloudProject = existingProject.current.value;
		} else {
			fetchedCloudProject = await cloudProjectService.createProject(
				$project.title,
				$project.description
			);
		}

		const mutableProject = structuredClone($project);
		mutableProject.api = {
			name: fetchedCloudProject.name,
			description: fetchedCloudProject.description,
			repository_id: fetchedCloudProject.repositoryId,
			git_url: fetchedCloudProject.gitUrl,
			code_git_url: fetchedCloudProject.codeGitUrl,
			created_at: fetchedCloudProject.createdAt,
			updated_at: fetchedCloudProject.updatedAt,
			sync: false,
			sync_code: false
		};
		await projectsService.updateProject(mutableProject);
	}

	async function onSyncChange(sync: boolean) {
		if (!$project?.api) return;

		const mutableProject = structuredClone($project);
		mutableProject.api!.sync = sync;
		projectsService.updateProject(mutableProject);
	}

	async function onSyncCodeChange(sync_code: boolean) {
		if (!$project?.api) return;

		const mutableProject = structuredClone($project);
		mutableProject.api!.sync_code = sync_code;
		projectsService.updateProject(mutableProject);
	}
</script>

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

	<Loading loadable={cloudProject.current}>
		{#snippet children(cloudProject)}
			{#if !cloudProject.parentProjectRepositoryId}
				<Section>
					{#snippet title()}
						Link your project with an organization
					{/snippet}

					<div bind:this={organizationsList}>
						{#each usersOrganizations.current as loadableOrganization, index}
							<SectionCard
								roundedBottom={index === usersOrganizations.current.length - 1}
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
		<Button onclick={createProject}>Enable cloud functionality</Button>
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

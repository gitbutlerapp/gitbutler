<script lang="ts">
	import ProjectConnectModal from '$components/ProjectConnectModal.svelte';
	import Section from '$components/Section.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { ProjectsService } from '$lib/project/projectsService';
	import { User } from '$lib/user/user';
	import { UserService } from '$lib/user/userService';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound, map } from '@gitbutler/shared/network/loadable';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizations } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import type { Project as BackendProject } from '$lib/project/project';
	import type { Project } from '@gitbutler/shared/organizations/types';
	const appState = getContext(AppState);
	const projectsService = getContext(ProjectsService);
	const projectService = getContext(ProjectService);
	const cloudProjectService = getContext(CloudProjectService);
	const organizationService = getContext(OrganizationService);
	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const userService = getContext(UserService);
	const webRoutes = getContext(WebRoutesService);

	const project = projectService.project;
	const userLogin = userService.userLogin;
	const user = getContextStore(User);

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
			sync_code: false,
			reviews: false
		};
		await projectsService.updateProject(mutableProject);
	}

	async function detachProject() {
		if (!$project) {
			return;
		}

		const mutableProject: BackendProject & { unset_api?: boolean } = structuredClone($project);
		mutableProject.api = undefined;
		mutableProject.unset_api = true;
		await projectsService.updateProject(mutableProject);
	}

	async function toggleProject() {
		if ($project?.api?.repository_id) {
			detachProject();
		} else {
			createProject();
		}
	}

	async function onReviewsChange(reviews: boolean) {
		if (!$project?.api) return;

		const mutableProject = structuredClone($project);
		mutableProject.api!.reviews = reviews;
		projectsService.updateProject(mutableProject);
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

<Section>
	<SectionCard orientation="row" labelFor="signCommits">
		{#snippet title()}
			GitButler Server Features
		{/snippet}
		{#snippet caption()}
			Enabling this allows you to turn on various hosted features for this project on gitbutler.com,
			including reviews, history sync, and branch sync.
		{/snippet}
		{#snippet actions()}
			<Toggle id="signCommits" checked={!!$project?.api} onclick={toggleProject} />
		{/snippet}
	</SectionCard>
</Section>

{#if cloudProject}
	<br />
	<Section gap={0}>
		{#snippet children()}
			<SectionCard
				labelFor="reviews"
				orientation="row"
				roundedTop={true}
				roundedBottom={$user?.role !== 'admin'}
			>
				{#snippet title()}
					Use GitButler Review
				{/snippet}
				{#snippet caption()}
					Use GitButler Reviews with this project. Reviews is a commit based code review tool that
					helps your team review series of patches.
					<Link href="https://docs.gitbutler.com/review/overview">Learn more</Link>
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="reviews"
						checked={$project?.api?.reviews || false}
						onclick={async () => await onReviewsChange(!$project?.api?.reviews)}
					/>
				{/snippet}
			</SectionCard>
			{#if $user?.role === 'admin'}
				<SectionCard
					labelFor="historySync"
					roundedBottom={false}
					roundedTop={false}
					orientation="row"
				>
					{#snippet title()}
						Timeline Backup
					{/snippet}
					{#snippet caption()}
						Sync this project's operations log (timeline) to GitButler servers. The operations log
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
				<SectionCard labelFor="branchesySync" roundedTop={false} orientation="row">
					{#snippet title()}
						Code Hosting
					{/snippet}
					{#snippet caption()}
						Push this project's branches to a hosted GitButler repository.
					{/snippet}
					{#snippet actions()}
						<Toggle
							id="branchesySync"
							checked={$project?.api?.sync_code || false}
							onclick={async () => await onSyncCodeChange(!$project?.api?.sync_code)}
						/>
					{/snippet}
				</SectionCard>
			{/if}

			<br />

			<Loading loadable={cloudProject.current}>
				{#snippet children(cloudProject)}
					<div class="api-link text-12">
						<Link
							target="_blank"
							rel="noreferrer"
							href={webRoutes.projectUrl({
								ownerSlug: cloudProject.owner,
								projectSlug: cloudProject.slug
							})}>Go to GitButler Server Project</Link
						>
					</div>
				{/snippet}
			</Loading>
		{/snippet}
	</Section>

	{#if $user?.role === 'admin'}
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
	{/if}
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

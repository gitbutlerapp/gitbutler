<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { organizationsSelectors } from '@gitbutler/shared/organizations/organizationsSlice';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { projectsSelectors } from '@gitbutler/shared/organizations/projectsSlice';
	import { AppState } from '@gitbutler/shared/redux/store';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	type Props = {
		organizationSlug: string;
		projectRepositoryId: string;
	};

	const { organizationSlug, projectRepositoryId }: Props = $props();

	const organizationService = getContext(OrganizationService);
	const projectsService = getContext(ProjectService);
	const appState = getContext(AppState);

	const organizationsState = appState.organizations;
	const projectsState = appState.projects;

	const organizationInterest = $derived(
		organizationService.getOrganizationWithDetailsInterest(organizationSlug)
	);
	const projectInterest = $derived(projectsService.getProjectInterest(projectRepositoryId));

	const organization = $derived(
		organizationsSelectors.selectById($organizationsState, organizationSlug)
	);
	const project = $derived(projectsSelectors.selectById($projectsState, projectRepositoryId));

	const organizationProjects = $derived(
		organization?.projectRepositoryIds?.map((repositoryId) => ({
			project: projectsSelectors.selectById($projectsState, repositoryId),
			projectInterest: projectsService.getProjectInterest(repositoryId)
		})) || []
	);

	function connectToOrganization(projectSlug?: string) {
		if (!project || !organization) return;

		projectsService.connectProjectToOrganization(
			project.repositoryId,
			organization.slug,
			projectSlug
		);
	}

	let modal = $state<Modal>();
</script>

<Modal bind:this={modal} title={`Join ${project?.name} into ${organization?.name}`}>
	<RegisterInterest interest={organizationInterest} />
	<RegisterInterest interest={projectInterest} />

	{#if organization}
		{#each organizationProjects as { project, projectInterest }, index}
			<RegisterInterest interest={projectInterest} />

			<SectionCard
				roundedTop={index === 0}
				roundedBottom={index === organizationProjects.length - 1}
			>
				{#if project}
					<h5>{project.name}</h5>

					<Button onclick={() => connectToOrganization(project.slug)}>Connect</Button>
				{:else}
					<p>Loading...</p>
				{/if}
			</SectionCard>
		{/each}
	{/if}

	<Button onclick={() => connectToOrganization()}>Create organization project</Button>
</Modal>

<Button onclick={() => modal?.show()}>Connect</Button>

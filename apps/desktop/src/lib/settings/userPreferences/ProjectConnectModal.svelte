<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { organizationsSelectors } from '@gitbutler/shared/organizations/organizationsSlice';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { projectsSelectors } from '@gitbutler/shared/organizations/projectsSlice';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
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

	const organizationInterest = $derived(
		organizationService.getOrganizationWithDetailsInterest(organizationSlug)
	);
	const projectInterest = $derived(projectsService.getProjectInterest(projectRepositoryId));

	const chosenOrganization = $derived(
		organizationsSelectors.selectById(appState.organizations, organizationSlug)
	);
	const targetProject = $derived(
		projectsSelectors.selectById(appState.projects, projectRepositoryId)
	);

	const organizationProjects = $derived.by(() => {
		if (chosenOrganization?.status !== 'found') return [];
		return (
			chosenOrganization.value.projectRepositoryIds?.map((repositoryId) => ({
				project: projectsSelectors.selectById(appState.projects, repositoryId),
				interest: projectsService.getProjectInterest(repositoryId)
			})) || []
		);
	});

	function connectToOrganization(projectSlug?: string) {
		if (targetProject?.status !== 'found' || chosenOrganization?.status !== 'found') return;

		projectsService.connectProjectToOrganization(
			targetProject.value.repositoryId,
			chosenOrganization.value.slug,
			projectSlug
		);
	}

	const title = $derived.by(() => {
		if (targetProject?.status !== 'found' || chosenOrganization?.status !== 'found') return;

		return `Join ${targetProject.value.name} into ${chosenOrganization.value.name}`;
	});

	let modal = $state<Modal>();
</script>

<Modal bind:this={modal} {title}>
	<RegisterInterest interest={organizationInterest} />
	<RegisterInterest interest={projectInterest} />

	{#each organizationProjects as { project: organizationProject, interest }, index}
		<RegisterInterest {interest} />

		<SectionCard roundedTop={index === 0} roundedBottom={index === organizationProjects.length - 1}>
			<Loading loadable={organizationProject}>
				{#snippet children(organizationProject)}
					<h5>{organizationProject.name}</h5>

					<Button onclick={() => connectToOrganization(organizationProject.slug)}>Connect</Button>
				{/snippet}
			</Loading>
		</SectionCard>
	{/each}

	<Button onclick={() => connectToOrganization()}>Create organization project</Button>
</Modal>

<Button onclick={() => modal?.show()}>Connect</Button>

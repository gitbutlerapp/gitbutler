<script lang="ts">
	import { inject } from '@gitbutler/core/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';
	import { organizationTable } from '@gitbutler/shared/organizations/organizationsSlice';
	import { PROJECT_SERVICE } from '@gitbutler/shared/organizations/projectService';
	import { projectTable } from '@gitbutler/shared/organizations/projectsSlice';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { Button, Modal, SectionCard } from '@gitbutler/ui';

	type Props = {
		organizationSlug: string;
		projectRepositoryId: string;
	};

	const { organizationSlug, projectRepositoryId }: Props = $props();

	const organizationService = inject(ORGANIZATION_SERVICE);
	const projectsService = inject(PROJECT_SERVICE);
	const appState = inject(APP_STATE);

	const organizationInterest = $derived(
		organizationService.getOrganizationWithDetailsInterest(organizationSlug)
	);
	const projectInterest = $derived(projectsService.getProjectInterest(projectRepositoryId));

	const chosenOrganization = $derived(
		organizationTable.selectors.selectById(appState.organizations, organizationSlug)
	);
	const targetProject = $derived(
		projectTable.selectors.selectById(appState.projects, projectRepositoryId)
	);

	const organizationProjects = $derived.by(() => {
		if (chosenOrganization?.status !== 'found') return [];
		return (
			chosenOrganization.value.projectRepositoryIds?.map((repositoryId) => ({
				project: projectTable.selectors.selectById(appState.projects, repositoryId),
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

<script lang="ts">
	import { WebState } from '$lib/redux/store.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizations } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { projectTable } from '@gitbutler/shared/organizations/projectsSlice';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import toasts from '@gitbutler/ui/toasts';

	type Props = {
		projectRepositoryId: string;
	};

	const { projectRepositoryId }: Props = $props();

	const webState = getContext(WebState);
	const organizationService = getContext(OrganizationService);
	const projectService = getContext(ProjectService);

	const projectInterest = $derived(projectService.getProjectInterest(projectRepositoryId));
	const project = $derived(
		projectTable.selectors.selectById(webState.projects, projectRepositoryId)
	);

	// Get list of organizations the user belongs to
	const organizations = getOrganizations(webState, organizationService);

	async function connectToOrganization(organizationSlug: string, projectSlug?: string) {
		if (project?.status !== 'found') return;

		try {
			await projectService.connectProjectToOrganization(
				projectRepositoryId,
				organizationSlug,
				projectSlug
			);
			toasts.success('Project connected to organization');
			modal?.close();
		} catch (error) {
			toasts.error(
				`Failed to connect project: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		}
	}

	const title = $derived.by(() => {
		if (project?.status !== 'found') return 'Connect Project';
		return `Connect ${project.value.name} to an Organization`;
	});

	let modal = $state<ReturnType<typeof Modal>>();

	// Expose a show method that can be called from the parent component
	export function show() {
		modal?.show();
	}
</script>

<Modal bind:this={modal} {title}>
	<RegisterInterest interest={projectInterest} />

	{#if organizations.current && organizations.current.length > 0}
		<div class="organizations-list">
			{#each organizations.current as organization, index}
				<Loading loadable={organization}>
					{#snippet children(organization)}
						<SectionCard
							roundedTop={index === 0}
							roundedBottom={index === organizations.current.length - 1}
							orientation="row"
							centerAlign
						>
							<div class="org-info">
								<h5 class="text-15 text-bold">{organization.name || organization.slug}</h5>
								{#if organization.description}
									<p class="description">{organization.description}</p>
								{/if}
							</div>
							<Button style="pop" onclick={() => connectToOrganization(organization.slug)}>
								Connect
							</Button>
						</SectionCard>
					{/snippet}
				</Loading>
			{/each}
		</div>
	{:else}
		<div class="empty-state">
			<p>You don't belong to any organizations yet.</p>
			<p>Create or join an organization to connect this project.</p>
		</div>
	{/if}
</Modal>

<style lang="postcss">
	.organizations-list {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.org-info {
		flex: 1;
	}

	.description {
		color: var(--text-muted, #666);
		font-size: 0.9rem;
		margin-top: 4px;
	}

	.empty-state {
		text-align: center;
		padding: 24px 0;
		color: var(--text-muted, #666);
	}
</style>

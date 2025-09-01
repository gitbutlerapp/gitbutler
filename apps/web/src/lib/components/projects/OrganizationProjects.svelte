<script lang="ts">
	import ProjectIndexCard from '$lib/components/projects/ProjectIndexCard.svelte';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizationBySlug } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';

	type Props = {
		slug: string;
	};

	const appState = inject(APP_STATE);
	const organizationService = inject(ORGANIZATION_SERVICE);

	const { slug }: Props = $props();

	const organization = getOrganizationBySlug(appState, organizationService, slug);
</script>

<Loading loadable={organization.current}>
	{#snippet children(organization)}
		{#if organization.projectRepositoryIds}
			{#each organization.projectRepositoryIds as projectId, index}
				<ProjectIndexCard
					roundedTop={index === 0}
					roundedBottom={organization.projectRepositoryIds.length - 1 === index}
					{projectId}
				/>
			{/each}
		{/if}
	{/snippet}
</Loading>

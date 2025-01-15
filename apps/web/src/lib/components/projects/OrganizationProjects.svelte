<script lang="ts">
	import ProjectIndexCard from '$lib/components/projects/ProjectIndexCard.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizationBySlug } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	type Props = {
		slug: string;
	};

	const appState = getContext(AppState);
	const organizationService = getContext(OrganizationService);

	const { slug }: Props = $props();

	const organization = getOrganizationBySlug(appState, organizationService, slug);
</script>

<Loading loadable={organization.current}>
	{#snippet children(organization)}
		{#each organization.projectRepositoryIds || [] as projectId}
			<ProjectIndexCard {projectId} />
		{/each}
	{/snippet}
</Loading>

<script lang="ts">
	import { goto } from '$app/navigation';
	import OrganizationProfile from '$lib/components/OrganizationProfile.svelte';
	import UserProfile from '$lib/components/UserProfile.svelte';
	import { featureShowProjectPage } from '$lib/featureFlags';
	import { OWNER_SERVICE } from '$lib/owner/ownerService';
	import { inject } from '@gitbutler/shared/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import type { OwnerParameters } from '@gitbutler/shared/routing/webRoutes.svelte';

	const routes = inject(WEB_ROUTES_SERVICE);

	$effect(() => {
		if (!$featureShowProjectPage) {
			goto(routes.homePath());
		}
	});

	interface Props {
		data: OwnerParameters;
	}

	let { data }: Props = $props();

	const ownerService = inject(OWNER_SERVICE);

	// Create a reactive store for the owner data
	let ownerStore = $derived(ownerService.getOwner(data.ownerSlug));
	let owner = $derived($ownerStore);

	// Helper derived values for easier template usage
	let loading = $derived(owner.status === 'loading');
	let error = $derived(owner.status === 'error' ? owner.error : null);
	let ownerData = $derived(owner.status === 'found' ? owner.value : null);
</script>

{#if loading}
	<div class="loading">
		<p>Loading owner information...</p>
	</div>
{:else if error}
	<div class="error">
		<p>Error: {error}</p>
	</div>
{:else if ownerData}
	{#if ownerData.type === 'user'}
		<UserProfile user={ownerData.data} ownerSlug={data.ownerSlug} />
	{:else if ownerData.type === 'organization'}
		<OrganizationProfile organization={ownerData.data} ownerSlug={data.ownerSlug} />
	{:else if ownerData.type === 'not_found'}
		<div class="not-found">
			<h2>Not Found</h2>
			<p>The owner "{data.ownerSlug}" could not be found.</p>
		</div>
	{/if}
{:else}
	<div class="not-found">
		<h2>Not Found</h2>
		<p>The owner "{data.ownerSlug}" could not be found.</p>
	</div>
{/if}

<style>
	.loading,
	.error,
	.not-found {
		padding: 1rem;
		border-radius: 0.5rem;
		text-align: center;
	}

	.error,
	.not-found {
		background-color: rgba(255, 0, 0, 0.1);
	}
</style>

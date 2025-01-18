<script lang="ts">
	import LandingPage from '$home/LandingPage.svelte';
	import { AuthService } from '$lib/auth/authService';
	import UserDashboard from '$lib/components/UserDashboard.svelte';
	import { cleanBreadcrumbs } from '$lib/components/breadcrumbs/breadcrumbsContext.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { page } from '$app/stores';

	const authService = getContext(AuthService);
	const token = $derived(authService.token);

	$effect(cleanBreadcrumbs);
</script>

{#if !$token && $page.url.pathname === '/'}
	<LandingPage />
{:else}
	<UserDashboard />
{/if}

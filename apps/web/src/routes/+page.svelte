<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import LandingPage from './home/components/LandingPage.svelte';
	import UserDashboard from '$lib/components/UserDashboard.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { page } from '$app/stores';

	const authService = getContext(AuthService);
	const token = $derived(authService.token);
</script>

{#if !$token && $page.url.pathname === '/'}
	<LandingPage />
{:else}
	<UserDashboard />
{/if}

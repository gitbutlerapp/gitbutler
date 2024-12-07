<script lang="ts">
  import { AuthService } from '$lib/auth/authService';
	import LandingPage from '$lib/components/LandingPage.svelte';
	import UserPage from '$lib/components/UserPage.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { page } from '$app/stores';

  const authService = getContext(AuthService);
  const token = $derived(authService.token);
</script>

{#if !$token && $page.url.pathname === '/'}
	<LandingPage />
{:else}
	<section>
		<h1>Your Dashboard</h1>
		<p>You rock. Here is your stuff.</p>
		<UserPage />
	</section>
{/if}

<style>
	section {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		flex: 0.6;
	}
	h1 { 
		font-size: 5rem;
	}
</style>

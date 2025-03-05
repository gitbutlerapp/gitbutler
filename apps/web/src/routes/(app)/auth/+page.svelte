<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	// import { goto } from '$app/navigation';
	import { env } from '$env/dynamic/public';

	const authService = getContext(AuthService);
	const token = $derived(authService.tokenReadable);

	if ($token) {
		// goto('/');
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login?callback=${window.location.href}`;
	}
</script>

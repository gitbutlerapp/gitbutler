<script lang="ts">
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import { inject } from '@gitbutler/shared/context';
	// import { goto } from '$app/navigation';
	import { env } from '$env/dynamic/public';

	const authService = inject(AUTH_SERVICE);
	const token = $derived(authService.tokenReadable);

	if ($token) {
		// goto('/');
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login?callback=${window.location.href}`;
	}
</script>

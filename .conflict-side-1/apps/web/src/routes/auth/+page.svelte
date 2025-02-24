<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { env } from '$env/dynamic/public';

	const authService = getContext(AuthService);
	const token = $derived(authService.tokenReadable);

	function login() {
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login?callback=${window.location.origin}`;
	}

	if ($token) {
		window.location.href = '/';
	} else {
		login();
	}
</script>

<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import { getContext } from '@gitbutler/shared/context';
	import { env } from '$env/dynamic/public';

	const authService = getContext(AuthService);
	const token = $derived(authService.token);

	function login() {
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login`;
	}
	function logout() {
		authService.clearToken();
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/logout`;
	}

	if ($token) {
		logout();
	} else {
		login();
	}
</script>

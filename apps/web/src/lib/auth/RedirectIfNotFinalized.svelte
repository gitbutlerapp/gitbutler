<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';

	const routesService = inject(WEB_ROUTES_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = userService.user;
	const userEmail = $derived($user?.email);
	const userLogin = $derived($user?.login);
	const isLoggedIn = $derived($user !== undefined);
	// Logged in but missing email or login
	const accountNotFinalized = $derived(isLoggedIn && (!userEmail || !userLogin));
	let currentPathName = $state<string>(location.pathname);

	// Keep track of the current path name to avoid redirect loops
	afterNavigate((navigation) => {
		currentPathName = navigation.to?.url.pathname || '';
	});

	// If the user account is not finalized, redirect to the finalization page
	$effect(() => {
		if (accountNotFinalized && currentPathName !== routesService.finalizeAccountPath()) {
			goto(routesService.finalizeAccountPath());
		}
	});
</script>

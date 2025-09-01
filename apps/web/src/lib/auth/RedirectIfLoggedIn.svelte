<script lang="ts">
	import { goto } from '$app/navigation';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';

	const routesService = inject(WEB_ROUTES_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = userService.user;
	const isLoggedIn = $derived($user !== undefined);

	// If there is a user, redirect to the home page
	$effect(() => {
		if (isLoggedIn) {
			goto(routesService.homePath());
		}
	});
</script>

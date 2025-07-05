<script lang="ts">
	import { goto } from '$app/navigation';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';

	const routesService = getContext(WebRoutesService);
	const userService = getContext(UserService);
	const user = userService.user;
	const isLoggedIn = $derived($user !== undefined);

	// If there is a user, redirect to the home page
	$effect(() => {
		if (isLoggedIn) {
			goto(routesService.homePath());
		}
	});
</script>

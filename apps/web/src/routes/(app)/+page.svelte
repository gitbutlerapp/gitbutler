<script lang="ts">
	import RedirectIfLoggedIn from "$lib/auth/RedirectIfLoggedIn.svelte";
	import { USER_SERVICE } from "$lib/user/userService";
	import { inject } from "@gitbutler/core/context";
	import { WEB_ROUTES_SERVICE } from "@gitbutler/shared/routing/webRoutes.svelte";

	const userService = inject(USER_SERVICE);
	const routes = inject(WEB_ROUTES_SERVICE);
	const user = userService.user;

	const loggedIn = $derived($user !== undefined);
</script>

{#if !loggedIn}
	<p>Loading...</p>
{:else}
	<!-- For now, just redirect the user back to the  -->
	<RedirectIfLoggedIn fallback={routes.profilePath()} />
{/if}

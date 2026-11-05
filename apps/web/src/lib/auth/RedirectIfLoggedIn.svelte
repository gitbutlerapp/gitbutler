<script lang="ts">
	import { goto } from "$app/navigation";
	import { getPageBeforeLogin } from "$lib/auth/pageBeforeLogin";
	import { USER_SERVICE } from "$lib/user/userService";
	import { inject } from "@gitbutler/core/context";
	import { WEB_ROUTES_SERVICE } from "@gitbutler/shared/routing/webRoutes.svelte";

	interface Props {
		/** Where to go when no page was saved before logging in: the home page if not given. */
		fallback?: string;
	}

	const { fallback }: Props = $props();

	const routesService = inject(WEB_ROUTES_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = userService.user;
	const isLoggedIn = $derived($user !== undefined);

	// If there is a user, return to the page they signed in from, or the fallback
	$effect(() => {
		if (isLoggedIn) {
			goto(getPageBeforeLogin() ?? fallback ?? routesService.homePath());
		}
	});
</script>

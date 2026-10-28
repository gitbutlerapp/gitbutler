<script lang="ts">
	import { goto } from "$app/navigation";
	import { USER_SERVICE } from "$lib/user/userService";
	import { inject } from "@gitbutler/core/context";
	import { WEB_ROUTES_SERVICE } from "@gitbutler/shared/routing/webRoutes.svelte";
	import { env } from "$env/dynamic/public";

	interface Props {
		/** Where to send a reader who is already signed in, instead of the home page. */
		callback?: string;
	}

	let { callback }: Props = $props();

	const routesService = inject(WEB_ROUTES_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = userService.user;
	const isLoggedIn = $derived($user !== undefined);

	// If there is a user, redirect to where they were sent from, or else to the home page. The
	// API's login sends them on to the callback once it has checked their session there too.
	$effect(() => {
		if (!isLoggedIn) return;
		if (callback) {
			window.location.href = `${env.PUBLIC_APP_HOST}cloud/login?callback=${encodeURIComponent(callback)}`;
		} else {
			goto(routesService.homePath());
		}
	});
</script>

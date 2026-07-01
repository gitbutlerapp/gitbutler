<script lang="ts">
	import RedirectToProfileIfLoggedIn from "$lib/auth/RedirectToProfileIfLoggedIn.svelte";
	import { USER_SERVICE } from "$lib/user/userService";
	import { inject } from "@gitbutler/core/context";

	const userService = inject(USER_SERVICE);
	const user = userService.user;

	const loggedIn = $derived($user !== undefined);
</script>

{#if !loggedIn}
	<p>Loading...</p>
{:else}
	<!-- For now, just redirect the user back to the  -->
	<RedirectToProfileIfLoggedIn />
{/if}

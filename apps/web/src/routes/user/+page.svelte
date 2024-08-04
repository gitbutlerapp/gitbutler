<script lang="ts">
	import { createUserService } from '$lib/user/userService.svelte';

	const userService = createUserService();
	let userAvatarUrl = $state(userService.user?.picture);

	function handleImageLoadError() {
		userAvatarUrl = `https://unavatar.io/${userService.user?.email}`;
	}
</script>

<svelte:head>
	<title>GitButler | User</title>
</svelte:head>

{#if !userService?.user?.id}
	<p>Loading...</p>
{:else}
	<div class="user__wrapper">
		<div class="user__id">
			<img
				class="user__id--img"
				alt="User Avatar"
				width="48"
				src={userAvatarUrl}
				onerror={handleImageLoadError}
			/>
			<div class="user__id--name">{userService.user?.name}</div>
		</div>
		<div><b>Email</b>: {userService.user?.email}</div>
		<div><b>Joined</b>: {userService.user?.created_at}</div>
		<div><b>Supporter</b>: {userService.user?.supporter}</div>
	</div>
{/if}

<style>
	.user__wrapper {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.user__id {
		display: flex;
		align-items: center;
		gap: 0.5rem;

		.user__id--img {
			border-radius: 0.5rem;
		}
	}
</style>

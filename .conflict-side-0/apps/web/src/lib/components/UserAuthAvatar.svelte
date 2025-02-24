<script lang="ts">
	import { goto } from '$app/navigation';
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Icon, Tooltip } from '@gitbutler/ui';
	import type { User } from '$lib/user/userService';

	interface Props {
		user: User | undefined | null;
	}

	const { user }: Props = $props();
	const routes = inject(WEB_ROUTES_SERVICE);
</script>

<Tooltip text="Profile & Settings">
	<button
		type="button"
		class="user-btn"
		onclick={() => {
			goto(routes.profilePath());
		}}
	>
		{#if user?.picture}
			<img class="user-icon__image" src={user.picture} alt="" referrerpolicy="no-referrer" />
		{:else}
			<Icon name="profile" />
		{/if}
	</button>
</Tooltip>

<style lang="postcss">
	.user-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-button);
		height: var(--size-button);
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-core-ntrl-100);

		& img {
			width: 100%;
			height: 100%;
			object-fit: cover;
		}
	}
</style>

<script lang="ts">
	import { goto } from '$app/navigation';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/shared/context';
	import { Icon } from '@gitbutler/ui';

	interface Props {
		pop?: boolean;
		isNavCollapsed?: boolean;
	}

	const { pop = false, isNavCollapsed = false }: Props = $props();

	const user = inject(USER);
</script>

<button
	type="button"
	class="btn"
	class:pop
	onclick={async () => goto(newSettingsPath())}
	class:collapsed={isNavCollapsed}
>
	{#if !isNavCollapsed}
		<span class="name text-13 text-semibold">
			{#if $user}
				{$user.name || $user.email}
			{:else}
				Account
			{/if}
		</span>
	{/if}
	{#if $user?.picture}
		<img class="profile-picture" src={$user.picture} alt="Avatar" referrerpolicy="no-referrer" />
	{:else}
		<div class="anon-icon">
			<Icon name="profile" />
		</div>
	{/if}
</button>

<style lang="postcss">
	.btn {
		display: flex;
		align-items: center;

		height: var(--size-cta);
		padding: 6px 8px;
		overflow-x: hidden;
		gap: 8px;
		border-radius: var(--radius-m);

		color: var(--clr-scale-ntrl-50);

		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast),
			filter var(--transition-fast);

		&.pop {
			background: var(--clr-scale-pop-70);
			color: var(--clr-scale-pop-10);

			&:hover {
				background: color-mix(in srgb, var(--clr-scale-pop-70) 90%, var(--clr-scale-pop-50));
				color: var(--clr-scale-pop-10);
			}
		}

		&:hover {
			background-color: var(--clr-bg-1-muted);
			color: var(--clr-scale-ntrl-40);
		}
	}
	.name {
		overflow-x: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.anon-icon,
	.profile-picture {
		width: 20px;
		height: 20px;
		border-radius: var(--radius-m);
	}
	.anon-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2px;
		background: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
	}

	/* MODIFIERS */
	.btn.collapsed {
		height: auto;
		padding: 8px;
		overflow-x: initial;

		& .anon-icon,
		.profile-picture {
			width: 24px;
			height: 24px;
		}
	}
</style>

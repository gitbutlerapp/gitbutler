<script lang="ts">
	import Icon from './Icon.svelte';
	import { User } from '$lib/stores/user';
	import { getContextStore } from '$lib/utils/context';
	import { goto } from '$app/navigation';

	export let pop = false;
	export let isNavCollapsed = false;

	const user = getContextStore(User);
</script>

<button
	class="btn"
	class:pop
	on:click={async () => await goto('/settings/')}
	class:collapsed={isNavCollapsed}
>
	{#if !isNavCollapsed}
		<span class="name text-base-13 text-semibold">
			{#if $user}
				{$user.name || $user.given_name || $user.email}
			{:else}
				Account
			{/if}
		</span>
	{/if}
	{#if $user?.picture}
		<img class="profile-picture" src={$user.picture} alt="Avatar" />
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
		overflow-x: hidden;
		gap: 8px;

		height: var(--size-cta);
		padding: 6px 8px;
		border-radius: var(--radius-m);

		color: var(--clr-scale-ntrl-50);
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast),
			filter var(--transition-fast);

		cursor: pointer;

		&.pop {
			color: var(--clr-scale-pop-10);
			background: var(--clr-scale-pop-70);

			&:hover {
				color: var(--clr-scale-pop-10);
				background: oklch(from var(--clr-scale-pop-70) calc(l - 0.03) c h);
			}
		}

		&:hover {
			background-color: var(--clr-bg-1-muted);
			color: var(--clr-scale-ntrl-40);
		}
	}
	.name {
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow-x: hidden;
	}
	.anon-icon,
	.profile-picture {
		border-radius: var(--radius-m);
		width: 20px;
		height: 20px;
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
		overflow-x: initial;
		padding: 8px;
		height: auto;

		& .anon-icon,
		.profile-picture {
			width: 24px;
			height: 24px;
		}
	}
</style>

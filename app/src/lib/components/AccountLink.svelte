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
		gap: var(--size-8);

		height: var(--size-cta);
		padding: var(--size-6) var(--size-8);
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
				background: oklch(from var(--clr-scale-pop-70) var(--hover-state-ratio) c h);
			}
		}

		&:hover {
			background-color: var(--clr-bg-2);
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
		width: var(--size-20);
		height: var(--size-20);
	}
	.anon-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--size-2);
		background: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
	}

	/* MODIFIERS */
	.btn.collapsed {
		overflow-x: initial;
		padding: var(--size-8);
		height: auto;

		& .anon-icon,
		.profile-picture {
			width: var(--size-24);
			height: var(--size-24);
		}
	}
</style>

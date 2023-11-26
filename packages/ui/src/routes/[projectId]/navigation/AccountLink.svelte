<script lang="ts">
	import { goto } from '$app/navigation';
	import type { User } from '$lib/backend/cloud';
	import Icon from '$lib/icons/Icon.svelte';

	export let user: User | undefined;
</script>

<button class="btn" on:click={() => goto('/user/')}>
	<span class="name text-base-13 text-semibold">
		{#if user}
			{user.name}
		{:else}
			Account
		{/if}
	</span>
	{#if user?.picture}
		<img class="profile-picture" src={user.picture} alt="Avatar" />
	{:else}
		<div class="anon-icon">
			<Icon name="profile" />
		</div>
	{/if}
</button>

<style lang="postcss">
	.btn {
		display: flex;
		color: var(--clr-theme-scale-ntrl-50);
		padding: var(--space-4) var(--space-8);
		border-radius: var(--radius-m);
		align-items: center;
		gap: var(--space-8);
		&:hover {
			background-color: var(--clr-theme-container-pale);
			color: var(--clr-theme-scale-ntrl-40);
		}
	}
	.anon-icon,
	.profile-picture {
		border-radius: var(--radius-m);
		width: var(--space-20);
		height: var(--space-20);
	}
	.anon-icon {
		padding: var(--space-2);
		background: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
	}
</style>

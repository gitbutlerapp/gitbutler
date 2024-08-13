<script lang="ts" context="module">
	export interface Props {
		avatars: {
			srcUrl: string;
			name: string;
		}[];
		maxAvatars?: number;
	}
</script>

<script lang="ts">
	import Avatar from './Avatar.svelte';
	import { tooltip } from '$lib/utils/tooltip';

	const { avatars, maxAvatars = 5 }: Props = $props();

	const maxTooltipLength = 10;
	const leftAvatars = $derived(avatars.length - maxAvatars);

	function getTooltipText() {
		if (leftAvatars <= maxTooltipLength) {
			return avatars
				.slice(maxAvatars)
				.map((avatar) => avatar.name)
				.join(', ');
		}

		if (leftAvatars > maxTooltipLength) {
			return (
				avatars
					.slice(maxAvatars, maxAvatars + maxTooltipLength)
					.map((avatar) => avatar.name)
					.join(', ') + ` and ${leftAvatars - maxTooltipLength} more`
			);
		}
	}
</script>

<div class="avatar-grouping">
	{#each avatars as avatar, i}
		{#if i < maxAvatars}
			<Avatar size="medium" srcUrl={avatar.srcUrl} tooltip={avatar.name} />
		{/if}
	{/each}
	{#if avatars.length > maxAvatars}
		<div
			class="avatars-counter"
			use:tooltip={{
				text: getTooltipText() || 'mr. unknown',
				delay: 500
			}}
		>
			<span class="text-base-11 text-semibold">+{avatars.length - maxAvatars}</span>
		</div>
	{/if}
</div>

<style lang="postcss">
	.avatar-grouping {
		position: relative;
		display: flex;

		& :global(> *) {
			margin-right: -4px;

			&:last-child {
				margin-right: 0;
			}
		}
	}

	.avatars-counter {
		user-select: none;
		z-index: var(--z-ground);
		position: relative;
		display: flex;
		justify-content: center;
		align-items: center;
		border-radius: 10px;
		padding: 0 4px;
		background-color: var(--clr-theme-ntrl-soft-hover);
		margin-left: 2px;

		& span {
			color: var(--clr-text-1);
			opacity: 0.8;
		}
	}
</style>

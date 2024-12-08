<script module lang="ts">
	export interface Props {
		avatars: {
			srcUrl: string;
			name: string;
		}[];
		maxAvatars?: number;
		size?: 'small' | 'medium' | 'large';
	}
</script>

<script lang="ts">
	import Avatar from './Avatar.svelte';
	import Tooltip from '$lib/Tooltip.svelte';

	const { avatars, maxAvatars = 3, size = 'medium' }: Props = $props();

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
			<Avatar {size} srcUrl={avatar.srcUrl} tooltip={avatar.name} />
		{/if}
	{/each}
	{#if avatars.length > maxAvatars}
		<Tooltip text={getTooltipText() || 'mr. unknown'}>
			<div class="avatars-counter">
				<span class="text-11 text-semibold">+{avatars.length - maxAvatars}</span>
			</div>
		</Tooltip>
	{/if}
</div>

<style lang="postcss">
	.avatar-grouping {
		position: relative;
		display: flex;

		& :global(> span) {
			display: flex;
			margin-right: -4px;
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

<script module lang="ts">
	import type { IconName } from '$lib/Icon.svelte';
	import type { ComponentColorType } from '$lib/utils/colorTypes';
	export interface Props {
		avatars: {
			srcUrl: string;
			name: string;
		}[];
		maxAvatars?: number;
		size?: 'small' | 'medium' | 'large';
		icon?: IconName;
		iconColor?: ComponentColorType;
	}
</script>

<script lang="ts">
	import Avatar from './Avatar.svelte';
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';

	const { avatars, maxAvatars = 3, size = 'medium', icon, iconColor }: Props = $props();

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

{#if avatars.length > 0}
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

		{#if avatars.length > 0 && icon}
			<div class="avatar-icon {iconColor}">
				<Icon name={icon} />
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.avatar-grouping {
		position: relative;
		display: flex;
		width: fit-content;

		& :global(> span) {
			display: flex;
			margin-right: -4px;
		}
	}

	.avatar-icon {
		position: absolute;
		top: -4px;
		right: -10px;

		z-index: var(--z-ground);
		width: 14px;
		height: 14px;

		display: flex;
		justify-content: center;
		align-items: center;
		border-radius: 50%;

		transform: scale(0.95);
		color: white;

		&.success {
			background: var(--clr-scale-succ-50);
		}

		&.error {
			background: var(--clr-scale-err-50);
		}

		&.pop {
			background: var(--clr-scale-pop-50);
		}

		&.warning {
			background: var(--clr-scale-warn-50);
		}

		&.purple {
			background: var(--clr-scale-purp-50);
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

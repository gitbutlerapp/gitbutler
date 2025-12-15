<script module lang="ts">
	export interface Props {
		avatars: {
			srcUrl: string;
			username: string;
		}[];
		maxAvatars?: number;
		size?: 'small' | 'medium' | 'large';
		icon?: IconName;
		iconColor?: ComponentColorType;
	}
</script>

<script lang="ts">
	import Icon, { type IconName } from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import Avatar from '$components/avatar/Avatar.svelte';
	import type { ComponentColorType } from '$lib/utils/colorTypes';

	const { avatars, maxAvatars = 3, size = 'medium', icon, iconColor }: Props = $props();

	const maxTooltipLength = 10;
	const leftAvatars = $derived(avatars.length - maxAvatars);

	function getTooltipText() {
		if (leftAvatars <= maxTooltipLength) {
			return avatars
				.slice(maxAvatars)
				.map((avatar) => avatar.username)
				.join(', ');
		}

		if (leftAvatars > maxTooltipLength) {
			return (
				avatars
					.slice(maxAvatars, maxAvatars + maxTooltipLength)
					.map((avatar) => avatar.username)
					.join(', ') + ` and ${leftAvatars - maxTooltipLength} more`
			);
		}
	}
</script>

{#if avatars.length > 0}
	<div class="avatar-grouping">
		{#each avatars as avatar, i}
			{#if i < maxAvatars}
				<Avatar
					{size}
					srcUrl={avatar.srcUrl}
					tooltip={avatar.username}
					username={avatar.username}
				/>
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
		display: flex;
		position: relative;
		width: fit-content;

		& :global(> span) {
			display: flex;
			margin-right: -4px;
		}

		& :global(> span:last-child) {
			margin-right: 0;
		}
	}

	.avatar-icon {
		display: flex;

		z-index: var(--z-ground);
		position: absolute;
		top: -4px;
		right: -10px;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;

		transform: scale(0.95);
		border-radius: 50%;
		color: white;

		&.success {
			background: var(--clr-theme-succ-element);
		}

		&.error {
			background: var(--clr-theme-danger-element);
		}

		&.pop {
			background: var(--clr-theme-pop-element);
		}

		&.warning {
			background: var(--clr-theme-warn-element);
		}

		&.purple {
			background: var(--clr-theme-purp-element);
		}
	}

	.avatars-counter {
		display: flex;
		z-index: var(--z-ground);
		position: relative;
		align-items: center;
		justify-content: center;
		margin-left: 2px;
		padding: 0 4px;
		border-radius: 10px;
		background-color: var(--clr-theme-gray-soft);
		user-select: none;

		& span {
			color: var(--clr-text-1);
			opacity: 0.8;
		}
	}
</style>

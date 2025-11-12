<script lang="ts">
	import { Icon, type IconName } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	type Props = {
		label: string;
		icon?: IconName;
		loading?: boolean;
		summary?: Snippet;
		content?: Snippet;
		expanded?: boolean;
		onToggle?: (expanded: boolean) => void;
		root?: boolean;
	};

	let {
		label,
		icon,
		loading = false,
		summary,
		content,
		expanded = $bindable(false),
		onToggle,
		root = false
	}: Props = $props();
</script>

<div class="expandable-section">
	<button
		type="button"
		class="section-header text-13"
		class:expanded
		onclick={() => {
			expanded = !expanded;
			onToggle?.(expanded);
		}}
	>
		<div class="flex items-center gap-6">
			<div class="section-header__arrow">
				<Icon name="chevron-right" />
			</div>
			{#if loading}
				<Icon name="spinner" />
			{:else if icon}
				<Icon name={icon} color="var(--clr-text-3)" />
			{/if}

			<span class="section-label" class:text-semibold={root}>{label}</span>
		</div>

		{#if summary && !(root && expanded)}
			<div class="section-summary">
				{@render summary()}
			</div>
		{/if}
	</button>

	{#if expanded && content}
		<div class="section-content">
			{@render content()}
		</div>
	{/if}
</div>

<style lang="postcss">
	.expandable-section {
		display: flex;
		flex-direction: column;
		max-width: 100%;
		overflow: hidden;
		gap: 12px;
		user-select: text;
	}

	.section-header {
		display: flex;
		position: relative;
		align-items: center;
		width: fit-content;
		gap: 10px;
		user-select: none;

		&:hover {
			.section-header__arrow {
				color: var(--clr-text-2);
			}
		}
	}

	.section-header__arrow {
		display: flex;
		margin-left: -2px;
		color: var(--clr-text-3);
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);
	}

	.expanded .section-header__arrow {
		transform: rotate(90deg);
	}

	.section-label {
		white-space: nowrap;
	}

	.section-summary {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.section-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
</style>

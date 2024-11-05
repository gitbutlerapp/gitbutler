<script lang="ts" module>
	import type { Snippet } from 'svelte';
	export interface Props {
		type: 'clear' | 'conflicted' | 'integrated';
		title: string;
		select: Snippet;
	}
</script>

<script lang="ts">
	import Icon from '$lib/Icon.svelte';

	let { type, title, select }: Props = $props();
</script>

<div class="integration-series-item no-select {type}">
	<div class="branch-icon">
		<Icon name="branch-small" />
	</div>
	<div class="name-label-wrap">
		<span class="name-label text-13 text-semibold truncate">
			{title}
		</span>

		<span class="name-label-badge text-11 text-semibold">
			{#if type === 'conflicted'}
				<span>Conflicted</span>
			{:else if type === 'integrated'}
				<span>Integrated</span>
			{/if}
		</span>
	</div>

	{#if select}
		<div class="select">
			{@render select()}
		</div>
	{/if}

	{#if type === 'integrated'}
		<div class="integrated-label-wrap">
			<Icon name="tick-small" />
			<span class="integrated-label text-12"> Part of the new base </span>
		</div>
	{/if}
</div>

<style lang="postcss">
	.integration-series-item {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 12px 12px 14px;
		min-height: 56px;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}

		.branch-icon {
			display: flex;
			align-items: center;
			justify-content: center;
			width: 16px;
			height: 16px;
			border-radius: var(--radius-s);
			color: var(--clr-core-ntrl-100);
		}

		/* NAME LABEL */
		.name-label-wrap {
			flex: 1;
			display: flex;
			align-items: center;
			gap: 8px;
			overflow: hidden;
		}

		.name-label {
			color: var(--clr-text-1);
		}

		.name-label-badge {
			padding: 2px 4px;
			border-radius: var(--radius-m);
			color: var(--clr-core-ntrl-100);
		}

		/* INTEGRATED LABEL */
		.integrated-label-wrap {
			display: flex;
			align-items: center;
			gap: 4px;
			padding-left: 6px;
			margin-right: 2px;
			color: var(--clr-text-2);
		}

		.integrated-label {
			white-space: nowrap;
		}

		.select {
			max-width: 130px;
		}

		/* MODIFIERS */
		&.clear {
			background-color: var(--clr-bg-1);

			.branch-icon {
				background-color: var(--clr-core-ntrl-50);
			}
		}

		&.conflicted {
			background-color: var(--clr-bg-1);

			.branch-icon,
			.name-label-badge {
				background-color: var(--clr-theme-warn-on-element);
				background-color: var(--clr-theme-warn-element);
			}
		}

		&.integrated {
			background-color: var(--clr-bg-1-muted);

			.branch-icon,
			.name-label-badge {
				color: var(--clr-theme-purp-on-element);
				background-color: var(--clr-theme-purp-element);
			}
		}
	}
</style>

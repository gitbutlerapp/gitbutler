<script lang="ts">
	import Tooltip from '$components/Tooltip.svelte';
	import ghLogo from '$lib/assets/review-badge/gh-logo.svg?raw';
	import glLogo from '$lib/assets/review-badge/gl-logo.svg?raw';

	interface Props {
		type: string | undefined;
		status?: 'open' | 'closed' | 'draft' | 'merged' | 'unknown';
		number?: number;
		title?: string;
	}

	const { type, status, number, title }: Props = $props();

	const reviewUnit = $derived(type === 'MR' ? 'MR' : 'PR');

	function getBadgeDetails() {
		if (title) {
			return {
				text: title,
				color: undefined
			};
		}

		switch (status) {
			case 'open':
				return {
					text: `${reviewUnit} #${number} is open`,
					color: 'var(--clr-theme-succ-element)'
				};
			case 'closed':
				return {
					text: `${reviewUnit} #${number} is closed`,
					color: 'var(--clr-theme-err-element)'
				};
			case 'draft':
				return {
					text: `${reviewUnit} #${number} is a draft`,
					color: undefined
				};
			case 'merged':
				return {
					text: `${reviewUnit} #${number} is merged`,
					color: 'var(--clr-theme-purp-element)'
				};
			default:
				return {
					text: `${reviewUnit} #${number}`,
					color: undefined
				};
		}
	}
</script>

<Tooltip text={getBadgeDetails().text}>
	<div class="review-badge" class:pr-type={status}>
		{#if type === 'MR'}
			{@html glLogo}
		{:else if type === 'PR'}
			{@html ghLogo}
		{/if}

		<span class="text-10 text-semibold review-badge-text">
			{#if status === 'draft'}
				Draft
			{:else}
				{reviewUnit}
			{/if}
		</span>

		{#if getBadgeDetails().color}
			<div class="pr-status" style:--pr-color={getBadgeDetails().color}></div>
		{/if}
	</div>
</Tooltip>

<style lang="postcss">
	.review-badge {
		display: flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		height: var(--size-icon);
		padding-right: 3px;
		padding-left: 2px;
		gap: 3px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1-muted);
		color: var(--clr-text-1);
		line-height: 1;
	}

	.pr-status {
		width: 8px;
		height: 8px;
		border-radius: 100%;
		background-color: var(--pr-color);
	}
</style>

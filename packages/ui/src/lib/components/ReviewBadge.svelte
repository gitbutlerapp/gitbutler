<script lang="ts">
	import Tooltip from '$components/Tooltip.svelte';
	import ghLogo from '$lib/assets/review-badge/gh-logo.svg?raw';
	import glLogo from '$lib/assets/review-badge/gl-logo.svg?raw';

	interface Props {
		type: string | undefined;
		status?: 'open' | 'closed' | 'draft' | 'merged' | 'unknown';
		number: number;
		title?: string;
	}

	const { type, status, number, title }: Props = $props();

	const reviewUnit = $derived(type === 'MR' ? 'MR' : 'PR');
	const reviewSymbol = $derived(reviewUnit === 'MR' ? '!' : '#');
	const id = $derived(`${reviewSymbol}${number}`);

	const badgeDetails = $derived.by(() => {
		if (title) {
			return {
				text: title,
				color: undefined
			};
		}

		switch (status) {
			case 'open':
				return {
					text: `${reviewUnit} ${id} is open`,
					color: 'var(--clr-theme-safe-element)'
				};
			case 'closed':
				return {
					text: `${reviewUnit} ${id} is closed`,
					color: 'var(--clr-theme-danger-element)'
				};
			case 'draft':
				return {
					text: `${reviewUnit} ${id} is a draft`,
					color: undefined
				};
			case 'merged':
				return {
					text: `${reviewUnit} ${id} is merged`,
					color: 'var(--clr-theme-purp-element)'
				};
			default:
				return {
					text: `${reviewUnit} ${id}`,
					color: undefined
				};
		}
	});
</script>

<Tooltip text={badgeDetails.text}>
	<div class="review-badge" class:pr-type={status}>
		<div class="review-badge__icon">
			{#if type === 'MR'}
				{@html glLogo}
			{:else if type === 'PR'}
				{@html ghLogo}
			{/if}
		</div>

		<span class="text-11 text-semibold review-badge-text">
			{#if status === 'draft'}
				Draft
			{:else}
				{reviewUnit} {id}
			{/if}
		</span>

		{#if badgeDetails.color}
			<div class="pr-status" style:--pr-color={badgeDetails.color}></div>
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

	.review-badge__icon {
		display: flex;
		flex-shrink: 0;
	}

	.review-badge-text {
		white-space: nowrap;
	}

	.pr-status {
		width: 8px;
		height: 8px;
		border-radius: 100%;
		background-color: var(--pr-color);
	}
</style>

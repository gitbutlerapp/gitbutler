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
					color: 'var(--clr-theme-purple-element)'
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
	<div class="review-badge pr-{status}">
		<div class="review-badge__icon">
			{#if type === 'MR'}
				{@html glLogo}
			{:else if type === 'PR'}
				{@html ghLogo}
			{/if}
		</div>

		<span class="text-11 text-semibold review-badge-text">
			{#if status === 'draft'}
				Draft {reviewUnit}
			{:else}
				{reviewUnit} {id}
			{/if}
		</span>
	</div>
</Tooltip>

<style lang="postcss">
	.review-badge {
		display: flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		height: var(--size-icon);
		padding-right: 5px;
		padding-left: 4px;
		gap: 3px;
		border-radius: var(--radius-ml);
		line-height: 1;
	}

	.review-badge__icon {
		display: flex;
		flex-shrink: 0;
	}

	.review-badge-text {
		white-space: nowrap;
	}

	.pr-open,
	.pr-unknown {
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-muted);
		color: var(--clr-text-1);
	}

	.pr-closed {
		background-color: var(--clr-theme-danger-soft);
		color: var(--clr-theme-danger-on-soft);
	}

	.pr-draft {
		border: 1px solid var(--clr-border-1);
		border-style: dotted;
		background-color: var(--clr-bg-muted);
		color: var(--clr-text-1);
	}

	.pr-merged {
		background-color: var(--clr-theme-purple-soft);
		color: var(--clr-theme-purple-on-soft);
	}
</style>

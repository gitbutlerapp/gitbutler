<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import ghLogo from '$lib/assets/review-badge/gh-logo.svg?raw';

	interface Props {
		prStatus?: 'open' | 'closed' | 'draft' | 'merged' | 'unknown';
		prNumber?: number;
		prTitle?: string;
		brStatus?: 'approved' | 'unreviewed' | 'changes_requested' | 'in-discussion' | 'unknown';
		brId?: string;
	}

	const { prStatus, prNumber, prTitle, brStatus, brId }: Props = $props();

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
					text: `PR #${prNumber} is open`,
					color: 'var(--clr-theme-succ-element)'
				};
			case 'closed':
				return {
					text: `PR #${prNumber} is closed`,
					color: 'var(--clr-theme-err-element)'
				};
			case 'draft':
				return {
					text: `PR #${prNumber} is a draft`,
					color: undefined
				};
			case 'merged':
				return {
					text: `PR #${prNumber} is merged`,
					color: 'var(--clr-theme-purp-element)'
				};
			default:
				return {
					text: `PR #${prNumber}`,
					color: undefined
				};
		}
	}
</script>

<Tooltip text={prNumber ? getPrBadgeDetails().text : getBrBadgeDetails().text}>
	<div class="review-badge" class:pr-type={prStatus} class:br-type={brStatus}>
		{#if prNumber}
			{@const prBadgeDetails = getPrBadgeDetails()}
			{@html ghLogo}

			<span class="text-10 text-semibold review-badge-text">
				{#if prStatus === 'draft'}
					Draft
				{:else}
					PR
				{/if}
			</span>

			{#if prBadgeDetails.color}
				<div class="pr-status" style:--pr-color={prBadgeDetails.color}></div>
			{/if}
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

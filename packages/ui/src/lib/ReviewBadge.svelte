<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import brApprovedSVG from '$lib/assets/review-badge/br-approved.svg?raw';
	import brChangesRequestedSVG from '$lib/assets/review-badge/br-changes-requested.svg?raw';
	import brInDiscussionSVG from '$lib/assets/review-badge/br-in-discussion.svg?raw';
	import brUnreviewedSVG from '$lib/assets/review-badge/br-unreviewed.svg?raw';
	import gbLogo from '$lib/assets/review-badge/gb-logo.svg?raw';
	import ghLogo from '$lib/assets/review-badge/gh-logo.svg?raw';

	interface Props {
		prStatus?: 'open' | 'closed' | 'draft' | 'merged' | 'unknown';
		prNumber?: number;
		prTitle?: string;
		brStatus?: 'approved' | 'unreviewed' | 'changes_requested' | 'in-discussion' | 'unknown';
		brId?: string;
	}

	const { prStatus, prNumber, prTitle, brStatus, brId }: Props = $props();

	function getPrBadgeDetails() {
		if (prTitle) {
			return {
				text: prTitle,
				color: undefined
			};
		}

		switch (prStatus) {
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

	function getBrBadgeDetails() {
		switch (brStatus) {
			case 'approved':
				return {
					text: `BR #${brId} is approved`,
					icon: brApprovedSVG
				};
			case 'unreviewed':
				return {
					text: `BR #${brId} is unreviewed`,
					icon: brUnreviewedSVG
				};
			case 'changes_requested':
				return {
					text: `BR #${brId} has changes requested`,
					icon: brChangesRequestedSVG
				};
			case 'in-discussion':
				return {
					text: `BR #${brId} is in discussion`,
					icon: brInDiscussionSVG
				};
			default:
				return {
					text: `BR #${brId}`,
					icon: undefined
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

		{#if brId}
			{@const brBadgeDetails = getBrBadgeDetails()}
			{@html gbLogo}

			<span class="text-10 text-semibold review-badge-text">BR</span>

			{#if brBadgeDetails.icon}
				{@html brBadgeDetails.icon}
			{/if}
		{/if}
	</div>
</Tooltip>

<style lang="postcss">
	.review-badge {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 4px;
		border-radius: var(--radius-ml);
		width: fit-content;
		height: var(--size-icon);
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1-muted);
		line-height: 1;
		color: var(--clr-text-1);

		&.pr-type {
			padding-left: 2px;
			padding-right: 3px;
		}

		&.br-type {
			padding-left: 4px;
			padding-right: 4px;
		}
	}

	.pr-status {
		width: 8px;
		height: 8px;
		border-radius: 100%;
		background-color: var(--pr-color);
	}
</style>

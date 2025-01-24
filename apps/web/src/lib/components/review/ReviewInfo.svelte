<script lang="ts">
	import ChangeStatus from '../changes/ChangeStatus.svelte';
	import {
		getPatchContributorsWithAvatars,
		getPatchReviewersWithAvatars,
		type Patch
	} from '@gitbutler/shared/branches/types';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';

	const NO_REVIEWERS = 'Not reviewed yet';
	const NO_CONTRIBUTORS = 'No contributors';
	const NO_COMMENTS = 'No comments yet';

	interface Props {
		patch: Patch;
	}

	const { patch }: Props = $props();

	const contributors = $derived(getPatchContributorsWithAvatars(patch));
	const reviewers = $derived(getPatchReviewersWithAvatars(patch));
</script>

<div class="review-main-content-info">
	<div class="review-main-content-info__entry">
		<p class="review-main-content-info__header">Status:</p>
		<ChangeStatus {patch} />
	</div>

	<div class="review-main-content-info__entry">
		<p class="review-main-content-info__header">Reviewed by:</p>
		<div>
			{#await reviewers then reviewers}
				{#if reviewers.length === 0}
					<p class="review-main-content-info__value">{NO_REVIEWERS}</p>
				{:else}
					<AvatarGroup avatars={reviewers}></AvatarGroup>
				{/if}
			{/await}
		</div>
	</div>

	<div class="review-main-content-info__entry">
		<p class="review-main-content-info__header">Commented by:</p>
		<p class="review-main-content-info__value">{NO_COMMENTS}</p>
	</div>

	<div class="review-main-content-info__entry">
		<p class="review-main-content-info__header">Authors:</p>
		<div>
			{#await contributors then contributors}
				{#if contributors.length === 0}
					<p class="review-main-content-info__value">{NO_CONTRIBUTORS}</p>
				{:else}
					<AvatarGroup avatars={contributors}></AvatarGroup>
				{/if}
			{/await}
		</div>
	</div>
</div>

<style>
	.review-main-content-info {
		display: flex;
		gap: 30px;
	}

	.review-main-content-info__entry {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.review-main-content-info__header {
		overflow: hidden;
		color: var(--text-2, #867e79);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}

	.review-main-content-info__value {
		overflow: hidden;
		color: var(--text-3, #b4afac);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}
</style>

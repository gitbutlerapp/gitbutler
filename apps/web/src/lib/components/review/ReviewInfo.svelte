<script lang="ts">
	import ChangeStatus from '../changes/ChangeStatus.svelte';
	import {
		getUsersWithAvatars,
		getPatchApproversWithAvatars,
		getPatchContributorsWithAvatars,
		getPatchRejectorsWithAvatars,
		type Patch
	} from '@gitbutler/shared/branches/types';
	import { getChatChannelParticipants } from '@gitbutler/shared/chat/chatChannelsPreview.svelte';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';

	const NO_REVIEWERS = 'Not reviewed yet';
	const NO_CONTRIBUTORS = 'No contributors';
	const NO_COMMENTS = 'No comments yet';

	interface Props {
		projectId: string;
		patch: Patch;
	}

	const { patch, projectId }: Props = $props();
	const appState = getContext(AppState);
	const chatChannelService = getContext(ChatChannelsService);

	const chatParticipants = $derived(
		getChatChannelParticipants(appState, chatChannelService, projectId, patch.changeId)
	);

	const commenters = $derived(
		chatParticipants.current === undefined
			? Promise.resolve([])
			: getUsersWithAvatars(chatParticipants.current)
	);
	const contributors = $derived(getPatchContributorsWithAvatars(patch));
	const approvers = $derived(getPatchApproversWithAvatars(patch));
	const rejectors = $derived(getPatchRejectorsWithAvatars(patch));
</script>

<div class="review-main-content-info">
	<div class="review-main-content-info__entry">
		<p class="text-12 review-main-content-info__header">Status:</p>
		<ChangeStatus {patch} />
	</div>

	<div class="review-main-content-info__entry">
		<p class="text-12 review-main-content-info__header">Reviewed by:</p>
		<div class="review-main-content-info__value-wrapper">
			{#await Promise.all([approvers, rejectors]) then [approvers, rejectors]}
				{#if approvers.length === 0 && rejectors.length === 0}
					<p class="review-main-content-info__value">{NO_REVIEWERS}</p>
				{:else}
					<AvatarGroup
						avatars={rejectors}
						maxAvatars={2}
						icon="refresh-small"
						iconColor="warning"
					/>
					<AvatarGroup avatars={approvers} maxAvatars={2} icon="tick-small" iconColor="success" />
				{/if}
			{/await}
		</div>
	</div>

	<div class="review-main-content-info__entry">
		<p class="text-12 review-main-content-info__header">Commented by:</p>
		{#await commenters then commentors}
			{#if commentors.length === 0}
				<p class="review-main-content-info__value">{NO_COMMENTS}</p>
			{:else}
				<AvatarGroup avatars={commentors} />
			{/if}
		{/await}
	</div>

	<div class="review-main-content-info__entry">
		<p class="text-12 review-main-content-info__header">Authors:</p>
		<div>
			{#await contributors then contributors}
				{#if contributors.length === 0}
					<p class="text-12 review-main-content-info__value">{NO_CONTRIBUTORS}</p>
				{:else}
					<AvatarGroup avatars={contributors} />
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
		color: var(--clr-text-2);
		text-overflow: ellipsis;
	}

	.review-main-content-info__value-wrapper {
		display: flex;
		gap: 8px;
	}

	.review-main-content-info__value {
		overflow: hidden;
		color: var(--clr-text-3);
		text-overflow: ellipsis;
	}
</style>

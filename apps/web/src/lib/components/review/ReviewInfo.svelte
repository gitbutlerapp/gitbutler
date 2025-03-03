<script lang="ts">
	import ChangeStatus from '../changes/ChangeStatus.svelte';
	import Factoid from '../infoFlexRow/Factoid.svelte';
	import InfoFlexRow from '../infoFlexRow/InfoFlexRow.svelte';
	import { getChatChannelParticipants } from '@gitbutler/shared/chat/chatChannelsPreview.svelte';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import {
		getUsersWithAvatars,
		getPatchApproversWithAvatars,
		getPatchContributorsWithAvatars,
		getPatchRejectorsWithAvatars
	} from '@gitbutler/shared/contributors';
	import { type Patch } from '@gitbutler/shared/patches/types';
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

<InfoFlexRow>
	<Factoid label="Status">
		<ChangeStatus {patch} />
	</Factoid>
	<Factoid label="Reviewed by" placeholderText={NO_REVIEWERS}>
		{#await Promise.all([approvers, rejectors]) then [approvers, rejectors]}
			{#if approvers.length > 0 || rejectors.length > 0}
				<AvatarGroup avatars={rejectors} maxAvatars={2} icon="refresh-small" iconColor="warning" />
				<AvatarGroup avatars={approvers} maxAvatars={2} icon="tick-small" iconColor="success" />
			{/if}
		{/await}
	</Factoid>
	<Factoid label="Commented by" placeholderText={NO_COMMENTS}>
		{#await commenters then commentors}
			{#if commentors.length > 0}
				<AvatarGroup avatars={commentors} />
			{/if}
		{/await}
	</Factoid>
	<Factoid label="Authors" placeholderText={NO_CONTRIBUTORS}>
		{#await contributors then contributors}
			{#if contributors.length > 0}
				<AvatarGroup avatars={contributors} />
			{/if}
		{/await}
	</Factoid>
</InfoFlexRow>

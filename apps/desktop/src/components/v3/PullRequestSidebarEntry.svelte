<script lang="ts">
	import SidebarEntry from '$components/v3/SidebarEntry.svelte';
	import { UserService } from '$lib/user/userService';
	import { parseDate } from '$lib/utils/time';
	import { inject } from '@gitbutler/shared/context';
	import type { PullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		pullRequest: PullRequest;
		selected: boolean;
		onclick: (listing: PullRequest) => void;
	}

	const { pullRequest, selected, onclick }: Props = $props();

	const [userService] = inject(UserService);
	const user = userService.user;

	const authorImgUrl = $derived.by(() => {
		return pullRequest.author?.email?.toLowerCase() === $user?.email?.toLowerCase()
			? $user?.picture
			: pullRequest.author?.gravatarUrl;
	});
</script>

<SidebarEntry
	prTitle={pullRequest.title}
	remotes={[]}
	local={false}
	applied={false}
	lastCommitDetails={{
		authorName: pullRequest.author?.name || 'Unknown',
		lastCommitAt: parseDate(pullRequest.modifiedAt)
	}}
	pullRequestDetails={pullRequest && {
		title: pullRequest.title,
		draft: pullRequest.draft
	}}
	onclick={() => onclick(pullRequest)}
	{selected}
	avatars={[
		{
			name: pullRequest.author?.name || 'unknown',
			srcUrl: authorImgUrl || ''
		}
	]}
/>

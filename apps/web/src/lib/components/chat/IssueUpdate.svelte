<script lang="ts">
	import { eventTimeStamp } from '@gitbutler/shared/branches/utils';

	import { Icon } from '@gitbutler/ui';
	import type { IssueUpdateEvent } from '@gitbutler/shared/patchEvents/types';

	const UNKNOWN_AUTHOR = 'Unknown author';

	interface Props {
		event: IssueUpdateEvent;
	}

	const { event }: Props = $props();

	const issueUpdate = $derived(event.object);
	const user = $derived(event.user);

	const authorName = $derived(user?.login ?? user?.name ?? user?.email ?? UNKNOWN_AUTHOR);

	const timestamp = $derived(eventTimeStamp(event));
</script>

<div class="issue-update">
	<div class="issue-update__header">
		{#if user}
			<img class="issue-update__avatar" src={user.avatarUrl} alt={authorName} />
		{/if}

		<div class="text-12 text-bold issue-update__author-name">{authorName}</div>

		{#if issueUpdate.resolved}
			<div class="issue-update__status-icon">
				<Icon name="tick-extrasmall" />
			</div>

			<p class="text-12 issue-update__status">resolved</p>
		{/if}

		<div class="text-12 issue-update__timestamp" title={event.createdAt}>{timestamp}</div>
	</div>
</div>

<style lang="postcss">
	.issue-update {
		display: flex;
		flex-direction: column;
		padding: 14px 16px;
		gap: 12px;
		border-bottom: 1px solid var(--clr-border-3);

		border-left: 4px solid var(--clr-theme-succ-element);
	}

	.issue-update__header {
		display: flex;
		gap: 8px;
	}

	.issue-update__avatar {
		width: 16px;
		height: 16px;
		border-radius: 20px;
	}

	.issue-update__author-name {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
	}

	.issue-update__status-icon {
		display: flex;
		align-items: center;
		width: 16px;

		border-radius: var(--radius-m);
		background: var(--clr-theme-succ-element);
		color: var(--clr-core-ntrl-100);
	}

	.issue-update__status {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
	}

	.issue-update__timestamp {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;

		opacity: 0.4;
	}
</style>

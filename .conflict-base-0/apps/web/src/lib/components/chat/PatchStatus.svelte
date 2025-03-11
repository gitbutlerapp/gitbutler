<script lang="ts">
	import { eventTimeStamp } from '@gitbutler/shared/branches/utils';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { PatchStatusEvent } from '@gitbutler/shared/patchEvents/types';

	const UNKNOWN_USER = 'Unknown User';

	interface Props {
		event: PatchStatusEvent;
	}

	const { event }: Props = $props();

	const userName = $derived(
		event.user?.login ?? event.user?.name ?? event.user?.email ?? UNKNOWN_USER
	);
	const statusAction = $derived(event.data.status ? 'approved' : 'requested changes on');
	const timestamp = $derived(eventTimeStamp(event));
</script>

<div class="patch-status" class:request-changes={!event.data.status}>
	<div class="patch-status__icon" class:request-changes={!event.data.status}>
		<Icon name={event.data.status ? 'confeti' : 'refresh-in-circle'} />
	</div>

	<div class="patch-status-content">
		<div class="patch-status__header">
			{#if event.user}
				<img class="patch-status__avatar" src={event.user.avatarUrl} alt={userName} />
			{/if}
			<p class="text-13 text-bold patch-status__name">{userName}</p>
			<p class="text-12 patch-status__message">{statusAction} this commit</p>
			<div class="text-12 patch-status__timestamp" title={event.createdAt}>{timestamp}</div>
		</div>

		{#if event.data.message}
			<p class="text-13 text-body patch-status__text-content">{event.data.message}</p>
		{/if}
	</div>
</div>

<style lang="postcss">
	.patch-status {
		display: flex;
		padding: 14px 16px;
		padding-left: 12px;
		gap: 12px;

		border-left: 4px solid var(--clr-theme-succ-element);
		border-bottom: 1px solid var(--clr-border-3);
		background: var(--clr-theme-succ-bg);

		&:first-child {
			border-bottom: none;
		}

		&.request-changes {
			border-left-color: var(--clr-br-commit-changes-requested-bg);
			background: var(--clr-bg-1);
		}
	}

	.patch-status__icon {
		display: flex;
		width: 24px;
		height: 24px;
		padding: 4px;
		justify-content: center;
		align-items: center;

		border-radius: 8px;
		background: var(--clr-theme-succ-element);
		color: var(--clr-core-ntrl-100);

		&.request-changes {
			background: var(--clr-br-commit-changes-requested-bg);
		}
	}

	.patch-status-content {
		display: flex;
		flex-direction: column;

		gap: 12px;
	}

	.patch-status__header {
		display: flex;
		align-items: center;
		padding-top: 4px;
		gap: 8px;
	}

	.patch-status__avatar {
		width: 16px;
		height: 16px;
		border-radius: 20px;
	}

	.patch-status__name {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
	}

	.patch-status__message {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
	}

	.patch-status__timestamp {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;

		opacity: 0.4;
	}

	.patch-status__text-content {
		color: var(--clr-text-1);
	}
</style>

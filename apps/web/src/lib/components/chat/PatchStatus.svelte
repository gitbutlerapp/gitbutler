<script lang="ts">
	import { eventTimeStamp } from '@gitbutler/shared/branches/utils';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { PatchStatusEvent } from '@gitbutler/shared/branches/types';

	const UNKNOWN_USER = 'Unknown User';

	interface Props {
		event: PatchStatusEvent;
	}

	const { event }: Props = $props();

	const userName = $derived(
		event.user?.login ?? event.user?.name ?? event.user?.email ?? UNKNOWN_USER
	);
	const statusAction = $derived(event.data.status ? 'approved' : 'rejected');
	const timestamp = $derived(eventTimeStamp(event));
</script>

<div class="patch-status">
	{#if event.data.status}
		<div class="patch-status__icon">
			<Icon name="confeti" />
		</div>
	{/if}

	<div class="patch-status-content">
		<div class="patch-status__header">
			{#if event.user}
				<img class="patch-status__avatar" src={event.user.avatarUrl} alt={userName} />
			{/if}
			<p class="patch-status__name">{userName}</p>
			<p class="patch-status__message">{statusAction} this commit</p>
			<div class="patch-status__timestamp" title={event.createdAt}>{timestamp}</div>
		</div>

		{#if event.data.message}
			<p class="patch-status__text-content">{event.data.message}</p>
		{/if}
	</div>
</div>

<style lang="postcss">
	.patch-status {
		display: flex;
		padding: 14px 16px;
		padding-left: 10px;
		gap: 12px;

		border-left: 4px solid var(--clr-theme-succ-element, #4ab582);
		border-bottom: 1px solid var(--clr-border-3, #eae9e8);
		background: var(--clr-theme-succ-bg, #f6fcfb);
	}

	.patch-status__icon {
		display: flex;
		width: 24px;
		height: 24px;
		padding: 4px;
		justify-content: center;
		align-items: center;

		border-radius: 8px;
		background: var(--clr-theme-succ-element, #4ab582);
		color: var(--clr-core-ntrl-100);
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
		color: var(--clr-text-1, #1a1614);
		text-overflow: ellipsis;

		/* base/13-bold */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 13px;
		font-style: normal;
		font-weight: 600;
		line-height: 120%; /* 15.6px */
	}

	.patch-status__message {
		overflow: hidden;
		color: var(--clr-text-1, #1a1614);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--text-weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}

	.patch-status__timestamp {
		overflow: hidden;
		color: var(--clr-text-1, #1a1614);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--text-weight-regular, 400);
		line-height: 120%; /* 14.4px */

		opacity: 0.4;
	}

	.patch-status__text-content {
		color: var(--clr-text-1, #1a1614);

		/* base-body/13 */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 13px;
		font-style: normal;
		font-weight: var(--text-weight-regular, 400);
		line-height: 160%; /* 20.8px */
	}
</style>

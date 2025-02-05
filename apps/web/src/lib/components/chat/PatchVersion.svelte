<script lang="ts">
	import {
		getPatchContributorsWithAvatars,
		type PatchVersionEvent
	} from '@gitbutler/shared/branches/types';
	import { eventTimeStamp, getMultipleContributorNames } from '@gitbutler/shared/branches/utils';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';

	interface Props {
		event: PatchVersionEvent;
	}

	const { event }: Props = $props();

	const patch = $derived(event.object);

	const authorNames = $derived(getMultipleContributorNames(patch.contributors));
	const authorAvatars = $derived(getPatchContributorsWithAvatars(patch));

	const timestamp = $derived(eventTimeStamp(event));
</script>

<div class="patch-version">
	<div class="patch-version__icon">
		<Icon name="patch" />
	</div>

	<div class="patch-version__header">
		{#if patch.contributors.length > 0}
			<div class="patch-version__author-avatars">
				{#await authorAvatars then contributors}
					<AvatarGroup avatars={contributors} />
				{/await}
			</div>
		{/if}

		<div class="patch-version__author-name">{authorNames}</div>

		<p class="patch-verssion__message">
			published a new <span>commit version #{patch.version}</span>
		</p>

		<div class="patch-version__timestamp" title={event.createdAt}>{timestamp}</div>
	</div>
</div>

<style lang="postcss">
	.patch-version {
		display: flex;
		align-items: center;
		padding: 14px 16px;
		padding-left: 12px;
		gap: 12px;

		border-left: 4px solid var(--clr-theme-pop-element, #3cb4ae);
		border-bottom: 1px solid var(--clr-border-3, #eae9e8);
		background: var(--clr-bg-1-muted, #f8f8f7);
	}

	.patch-version__icon {
		display: flex;
		width: 24px;
		height: 24px;
		padding: 4px;
		justify-content: center;
		align-items: center;
		flex-shrink: 0;
		border-radius: 8px;
		background: var(--clr-theme-pop-element, #3cb4ae);
		color: var(--clr-theme-pop-on-element, #ffffff);
	}

	.patch-version__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.patch-version__author-avatars {
		margin-right: 4px;
	}

	.patch-version__author-name {
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

	.patch-verssion__message {
		overflow: hidden;
		color: var(--clr-text-1, #1a1614);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--text-weight-regular, 400);
		line-height: 120%; /* 14.4px */

		span {
			overflow: hidden;
			-webkit-box-orient: vertical;
			-webkit-line-clamp: 1;
			color: var(--clr-text-1, #1a1614);
			text-overflow: ellipsis;

			/* base/12-bold */
			font-family: var(--text-fontfamily-default, Inter);
			font-size: 12px;
			font-style: normal;
			font-weight: var(--text-weight-bold, 500);
			line-height: 120%;
			text-decoration-line: underline;
			text-decoration-style: solid;
			text-decoration-skip-ink: none;
			text-decoration-thickness: auto;
			text-underline-offset: auto;
			text-underline-position: from-font;
		}
	}

	.patch-version__timestamp {
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
</style>

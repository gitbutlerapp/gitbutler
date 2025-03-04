<script lang="ts">
	import { eventTimeStamp, getMultipleContributorNames } from '@gitbutler/shared/branches/utils';
	import { getPatchContributorsWithAvatars } from '@gitbutler/shared/contributors';
	import { type PatchVersionEvent } from '@gitbutler/shared/patchEvents/types';
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

		<div class="text-13 text-bold patch-version__author-name">{authorNames}</div>

		<p class="text-12 patch-verssion__message">
			published a new <span class="text-bold">commit version #{patch.version}</span>
		</p>

		<div class="text-12 patch-version__timestamp" title={event.createdAt}>{timestamp}</div>
	</div>
</div>

<style lang="postcss">
	.patch-version {
		display: flex;
		align-items: center;
		padding: 14px 16px;
		padding-left: 12px;
		gap: 12px;

		border-left: 4px solid var(--clr-theme-pop-element);
		border-bottom: 1px solid var(--clr-border-3);
		background: var(--clr-bg-1-muted);
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
		background: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
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
		color: var(--clr-text-1);
		text-overflow: ellipsis;
	}

	.patch-verssion__message {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;

		span {
			overflow: hidden;
			-webkit-box-orient: vertical;
			-webkit-line-clamp: 1;
			color: var(--clr-text-1);
			text-overflow: ellipsis;

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
		color: var(--clr-text-1);
		text-overflow: ellipsis;

		opacity: 0.4;
	}
</style>

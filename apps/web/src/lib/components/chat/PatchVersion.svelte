<script lang="ts">
	import { setAfterVersion, setBeforeVersion } from '$lib/interdiffRangeQuery.svelte';
	import { eventTimeStamp, getMultipleContributorNames } from '@gitbutler/shared/branches/utils';
	import { getPatchContributorsWithAvatars } from '@gitbutler/shared/contributors';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { type PatchVersionEvent } from '@gitbutler/shared/patchEvents/types';
	import { getPatch } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import { AvatarGroup, Icon } from '@gitbutler/ui';

	interface Props {
		event: PatchVersionEvent;
	}

	const { event }: Props = $props();

	const patch = $derived(event.object);

	const authorNames = $derived(getMultipleContributorNames(patch.contributors));
	const authorAvatars = $derived(getPatchContributorsWithAvatars(patch));

	const timestamp = $derived(eventTimeStamp(event));
	const latestPatchCommit = $derived(getPatch(patch.branchUuid, patch.changeId));

	// NOTE: Because this is working with the query params this MUST NOT be
	// called if the `<ReviewSections>` of a different patchComit are currently
	// displayed. Doing so will cause it to show a potentially broken range.
	async function viewInterdiff() {
		if (!isFound(latestPatchCommit.current)) return;
		await setBeforeVersion(patch.version - 1);
		await setAfterVersion(latestPatchCommit.current.value.version, patch.version);
	}
</script>

<div class="patch-version">
	<div class="patch-version__icon">
		<Icon name="patch" />
	</div>

	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div class="patch-version__header">
		{#if patch.contributors.length > 0}
			<div class="patch-version__author-avatars">
				{#await authorAvatars then contributors}
					<AvatarGroup avatars={contributors} />
				{/await}
			</div>
		{/if}

		<div class="text-13 text-bold patch-version__author-name">{authorNames}</div>

		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<p class="text-12 patch-verssion__message" onclick={viewInterdiff}>
			published a new <span class="interdiff-version text-bold"
				>commit version #{patch.version}</span
			>
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
		border-bottom: 1px solid var(--clr-border-3);

		border-left: 4px solid var(--clr-theme-pop-element);
		background: var(--clr-bg-1-muted);
	}

	.patch-version__icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 4px;
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

			text-decoration-line: underline;
			text-decoration-style: solid;
			text-decoration-thickness: auto;
			text-decoration-skip-ink: none;
			text-underline-position: from-font;
			text-underline-offset: auto;
			text-overflow: ellipsis;
		}
	}

	.patch-version__timestamp {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;

		opacity: 0.4;
	}

	.interdiff-version {
		cursor: pointer;
	}
</style>

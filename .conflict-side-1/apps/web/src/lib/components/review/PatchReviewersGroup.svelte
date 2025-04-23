<script lang="ts">
	import {
		getPatchApproversAllWithAvatars,
		getPatchRejectorsAllWithAvatars
	} from '@gitbutler/shared/contributors';
	import { type PatchCommit } from '@gitbutler/shared/patches/types';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';

	type Props = {
		patchCommit: PatchCommit;
	};

	const { patchCommit }: Props = $props();

	const approvers = $derived(getPatchApproversAllWithAvatars(patchCommit));
	const rejectors = $derived(getPatchRejectorsAllWithAvatars(patchCommit));
</script>

{#await Promise.all([approvers, rejectors]) then [approvers, rejectors]}
	{#if approvers.length > 0 || rejectors.length > 0}
		<div class="reviewers-groups">
			<AvatarGroup avatars={rejectors} maxAvatars={2} icon="refresh-small" iconColor="warning" />
			<AvatarGroup avatars={approvers} maxAvatars={2} icon="tick-small" iconColor="success" />
		</div>
	{:else}
		<span class="row-placeholder">—</span>
	{/if}
{/await}

<style lang="postcss">
	.reviewers-groups {
		display: flex;
		gap: 10px;
	}
</style>

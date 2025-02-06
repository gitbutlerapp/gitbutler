<script lang="ts">
	import {
		getPatchApproversAllWithAvatars,
		getPatchRejectorsAllWithAvatars,
		type Patch
	} from '@gitbutler/shared/branches/types';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';

	type Props = {
		patch: Patch;
	};

	const { patch }: Props = $props();

	const approvers = $derived(getPatchApproversAllWithAvatars(patch));
	const rejectors = $derived(getPatchRejectorsAllWithAvatars(patch));
</script>

{#await Promise.all([approvers, rejectors]) then [approvers, rejectors]}
	{#if approvers.length > 0 || rejectors.length > 0}
		<AvatarGroup avatars={rejectors} maxAvatars={2} icon="refresh-small" iconColor="warning" />
		<AvatarGroup avatars={approvers} maxAvatars={2} icon="tick-small" iconColor="success" />
	{/if}
{/await}

<script lang="ts">
	import { getPatchViewersAllWithAvatars, type Patch } from '@gitbutler/shared/branches/types';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';

	type Props = {
		patch: Patch;
	};

	const { patch }: Props = $props();

	const avatars = $derived(getPatchViewersAllWithAvatars(patch));
</script>

{#await avatars then avatars}
	<div class="container"><AvatarGroup {avatars} /></div>
{/await}

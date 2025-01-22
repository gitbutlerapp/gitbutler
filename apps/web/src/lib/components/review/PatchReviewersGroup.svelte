<script lang="ts">
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
	import type { Patch } from '@gitbutler/shared/branches/types';

	type Props = {
		patch: Patch;
	};

	const { patch }: Props = $props();

	async function getContributorsWithAvatars(patch: Patch) {
		return await Promise.all(
			patch.reviewAll.viewed.map(async (contributor) => {
				return {
					srcUrl: await gravatarUrlFromEmail(contributor),
					name: contributor
				};
			})
		);
	}
	const avatars = $derived(getContributorsWithAvatars(patch));
</script>

{#await avatars then avatars}
	<div class="container"><AvatarGroup {avatars} /></div>
{/await}

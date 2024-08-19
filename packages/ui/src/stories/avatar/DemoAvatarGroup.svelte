<script lang="ts">
	import AvatarGroup from '$lib/avatar/AvatarGroup.svelte';

	interface Props {
		maxAvatars?: number;
		avatars: {
			srcUrl: string;
			name: string;
		}[];
	}

	const { avatars, maxAvatars = 3 }: Props = $props();
</script>

<AvatarGroup {avatars} {maxAvatars} />

<script lang="ts">
	import AvatarGrouping from '$lib/avatar/AvatarGrouping.svelte';

	interface Props {
		maxAvatars?: number;
		avatars: {
			srcUrl: string;
			name: string;
		}[];
	}

	const { avatars, maxAvatars = 3 }: Props = $props();
</script>

<AvatarGrouping {avatars} {maxAvatars} />

<script lang="ts">
	import Avatar from '$lib/avatar/Avatar.svelte';
	import AvatarGrouping from '$lib/avatar/AvatarGrouping.svelte';
	import SidebarEntry from '$lib/sidebarEntry/SidebarEntry.svelte';

	interface Props {
		selected?: boolean;
		title: string;
		applied?: boolean;
		pullRequestDetails?: { title: string };
		// Storybook can give us pretty much anything under the sun for a date so we need to handle it
		lastCommitDetails?: { authorName: string; lastCommitAt: any };
		branchDetails?: { commitCount: number; linesAdded: number; linesRemoved: number };
		remotes?: string[];
		local?: boolean;
	}

	const { ...args }: Props = $props();

	$effect.pre(() => {
		if (args.lastCommitDetails) {
			args.lastCommitDetails.lastCommitAt = new Date(args.lastCommitDetails.lastCommitAt);
		}
	});
</script>

<SidebarEntry {...args}>
	{#snippet authorAvatars()}
		<AvatarGrouping>
			<Avatar
				srcUrl="https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c"
				size="medium"
				tooltipText="Bestest hamster"
				altText="Bestest hamster"
			/>
			<Avatar
				srcUrl="https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c"
				size="medium"
				tooltipText="Bestest hamster"
				altText="Bestest hamster"
			/>
			<Avatar
				srcUrl="https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c"
				size="medium"
				tooltipText="Bestest hamster"
				altText="Bestest hamster"
			/>
		</AvatarGrouping>
	{/snippet}
</SidebarEntry>

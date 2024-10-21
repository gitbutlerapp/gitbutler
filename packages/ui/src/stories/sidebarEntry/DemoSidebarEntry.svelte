<script lang="ts">
	import SidebarEntry from '$lib/SidebarEntry.svelte';
	import AvatarGroup from '$lib/avatar/AvatarGroup.svelte';

	interface Props {
		selected?: boolean;
		title: string;
		applied?: boolean;
		pullRequestDetails?: { title: string; draft: boolean };
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
		<AvatarGroup
			avatars={[
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/76307?s=80&v=4',
					name: 'Sebastian Markbåge'
				},
				{
					srcUrl: 'https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c',
					name: 'Bestest hamster'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/869934?s=80&v=4',
					name: 'Benjamin den Boer'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/14818017?s=64&v=4',
					name: 'Paperstick'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/11708259?s=64&v=4',
					name: 'Andy Hook'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/1584370?s=60&v=4',
					name: 'Kombucha'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/25510810?s=60&v=4',
					name: 'Alberto Tonegari'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/357695?s=60&v=4',
					name: 'wallynm'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/9648559?s=60&v=4',
					name: 'csantos1113'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/57962793?s=60&v=4',
					name: 'imtiazmangerah'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/15154097?s=60&v=4',
					name: 'chungweileong94'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/88314186?s=60&v=4',
					name: 'Godhyeongman'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/3650909?s=60&v=4',
					name: 'Miguel Mota'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/88744?s=60&v=4',
					name: 'Jesse Duffield'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/3082153?s=60&v=4',
					name: 'chaance'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/23293719?s=60&v=4',
					name: 'rczobor'
				}
			]}
		/>
	{/snippet}
</SidebarEntry>

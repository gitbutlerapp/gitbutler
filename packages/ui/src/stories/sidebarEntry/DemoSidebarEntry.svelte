<script lang="ts">
	import SidebarEntry from '$lib/SidebarEntry.svelte';

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
		series: string[];
		avatars?: { name: string; srcUrl: string }[];
	}

	const { ...args }: Props = $props();

	$effect.pre(() => {
		if (args.lastCommitDetails) {
			args.lastCommitDetails.lastCommitAt = new Date(args.lastCommitDetails.lastCommitAt);
		}
	});
</script>

<SidebarEntry {...args} />

<script lang="ts">
	import LineGroup from '$lib/CommitLines/LineGroup.svelte';
	import { LineManager } from '$lib/CommitLines/lineManager';
	import type { CommitData } from '$lib/CommitLines/types';

	interface Props {
		remoteCommits: CommitData[];
		localCommits: CommitData[];
		localAndRemoteCommits: CommitData[];
		integratedCommits: CommitData[];
		sameForkpoint: boolean;
	}

	const {
		remoteCommits,
		localCommits,
		localAndRemoteCommits,
		integratedCommits,
		sameForkpoint
	}: Props = $props();

	const lineManager = $derived(
		new LineManager(
			{ remoteCommits, localCommits, localAndRemoteCommits, integratedCommits },
			sameForkpoint
		)
	);
</script>

<div class="column">
	{#each remoteCommits as commit}
		<div class="group">
			<LineGroup lineGroup={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each localCommits as commit}
		<div class="group">
			<LineGroup lineGroup={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each localAndRemoteCommits as commit}
		<div class="group">
			<LineGroup lineGroup={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each integratedCommits as commit}
		<div class="group">
			<LineGroup lineGroup={lineManager.get(commit.id)} />
		</div>
	{/each}

	<div class="group">
		<LineGroup lineGroup={lineManager.base} />
	</div>
</div>

<style lang="postcss">
	.group {
		height: 68px;
	}
</style>

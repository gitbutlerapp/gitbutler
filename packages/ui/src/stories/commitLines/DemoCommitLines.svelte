<script lang="ts">
	import Line from '$lib/commitLines/Line.svelte';
	import { LineManager } from '$lib/commitLines/lineManager';
	import type { CommitData } from '$lib/commitLines/types';

	interface Props {
		remoteCommits: CommitData[];
		localCommits: CommitData[];
		localAndRemoteCommits: CommitData[];
		integratedCommits: CommitData[];
	}

	const { remoteCommits, localCommits, localAndRemoteCommits, integratedCommits }: Props = $props();

	const lineManager = $derived(
		new LineManager({ remoteCommits, localCommits, localAndRemoteCommits, integratedCommits })
	);
</script>

<div class="column">
	{#each remoteCommits as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each localCommits as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each localAndRemoteCommits as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each integratedCommits as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}
</div>

<style lang="postcss">
	.group {
		height: 68px;
	}
</style>

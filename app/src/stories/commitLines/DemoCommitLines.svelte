<script lang="ts">
	import LineGroup from '$lib/commitLines/LineGroup.svelte';
	import { LineManager } from '$lib/commitLines/lineManager';
	import type { CommitData } from '$lib/commitLines/types.ts';

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
</div>

<style lang="postcss">
	.group {
		height: 68px;
	}
</style>

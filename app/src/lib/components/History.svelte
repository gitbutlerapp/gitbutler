<script lang="ts">
	import Button from './Button.svelte';
	import { invoke } from '$lib/backend/ipc';
	import { getContext } from '$lib/utils/context';
	import { toHumanReadableTime } from '$lib/utils/time';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { onMount } from 'svelte';

	export let projectId: string;

	const snapshotsLimit = 30;

	const vbranchService = getContext(VirtualBranchService);
	vbranchService.activeBranches.subscribe(() => {
		listSnapshots(projectId, snapshotsLimit);
	});

	type SnapshotEntry = {
		sha: string;
		label: string;
		createdAt: number;
	};
	let snapshots: SnapshotEntry[] = [];
	async function listSnapshots(projectId: string, limit: number) {
		const resp = await invoke<SnapshotEntry[]>('list_snapshots', {
			projectId: projectId,
			limit: limit
		});
		snapshots = resp;
	}
	async function restoreSnapshot(projectId: string, sha: string) {
		const resp = await invoke<string>('restore_snapshot', {
			projectId: projectId,
			sha: sha
		});
		console.log(resp);
	}
	onMount(async () => {
		listSnapshots(projectId, snapshotsLimit);
	});
</script>

<div class="container">
	{#each snapshots as entry, idx}
		<div class="card">
			<div class="entry">
				<div>
					{entry.label}
				</div>
				<div>
					<span>
						{toHumanReadableTime(entry.createdAt)}
					</span>
					{#if idx != 0}
						<Button
							style="pop"
							size="tag"
							icon="undo-small"
							on:click={async () => await restoreSnapshot(projectId, entry.sha)}>restore</Button
						>
					{/if}
				</div>
			</div>
		</div>
	{/each}
</div>

<style>
	.container {
		width: 50rem;
		padding: 0.5rem;
		border-left-width: 1px;
		overflow-y: auto;
	}
	.entry {
		flex: auto;
		flex-direction: column;
	}
</style>

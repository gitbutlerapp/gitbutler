<script lang="ts">
	import Button from './Button.svelte';
	import { invoke } from '$lib/backend/ipc';
	import { getContext } from '$lib/utils/context';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	export let projectId: string;

	const snapshotsLimit = 100;

	const vbranchService = getContext(VirtualBranchService);
	vbranchService.activeBranches.subscribe(() => {
		listSnapshots(projectId, snapshotsLimit);
	});

	type Trailer = {
		key: string;
		value: string;
	};
	type SnapshotDetails = {
		title: string;
		operation: string;
		body: string | undefined;
		trailers: Trailer[];
	};
	type Snapshot = {
		id: string;
		details: SnapshotDetails | undefined;
		createdAt: number;
	};
	let snapshots: Snapshot[] = [];
	async function listSnapshots(projectId: string, limit: number) {
		const resp = await invoke<Snapshot[]>('list_snapshots', {
			projectId: projectId,
			limit: limit
		});
		snapshots = resp;
	}
	async function restoreSnapshot(projectId: string, sha: string) {
		await invoke<string>('restore_snapshot', {
			projectId: projectId,
			sha: sha
		});
		// TODO: is there a better way to update all the state?
		await goto(window.location.href, { replaceState: true });
	}
	onMount(async () => {
		listSnapshots(projectId, snapshotsLimit);
	});
</script>

<div class="container">
	{#each snapshots as entry, idx}
		<div class="card" style="margin-bottom: 4px;">
			<div class="entry" style="padding: 6px;">
				<div style="display: flex; align-items: center;">
					<div>id: {entry.id.slice(0, 7)}</div>
					<div style="flex-grow: 1;" />
					<div>
						{#if idx != 0}
							<Button
								style="pop"
								size="tag"
								icon="undo-small"
								on:click={async () => await restoreSnapshot(projectId, entry.id)}>restore</Button
							>
						{:else}
							(current)
						{/if}
					</div>
				</div>
				<span>
					time: {new Date(entry.createdAt).toLocaleString()}
				</span>
				<div>
					type: <b>{entry.details?.operation}</b>
				</div>
				<div style="padding-left: 16px; hidden;">
					{#if entry.details?.operation === 'RestoreFromSnapshot'}
						from: {entry.details?.trailers
							.find((t) => t.key === 'restored_from')
							?.value?.slice(0, 7)}
					{:else if entry.details?.operation === 'FileChanges'}
						{#each entry.details?.trailers
							.find((t) => t.key === 'files')
							?.value?.split(',') || [] as file}
							<div>{file}</div>
						{/each}
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

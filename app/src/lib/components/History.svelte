<script lang="ts">
	import Button from './Button.svelte';
	import { invoke } from '$lib/backend/ipc';
	import { getContext } from '$lib/utils/context';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';

	export let projectId: string;

	const vbranchService = getContext(VirtualBranchService);
	vbranchService.activeBranches.subscribe(() => {
		// whenever virtual branches change, we need to reload the snapshots
		// TODO: if the list has results from more pages, merge into it?
		listSnapshots(projectId).then((rsp) => {
			snapshots = rsp;
			listElement?.scrollTo(0, 0);
		});
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
		linesAdded: number;
		linesRemoved: number;
		filesChanged: string[];
		details: SnapshotDetails | undefined;
		createdAt: number;
	};
	let snapshots: Snapshot[] = [];
	async function listSnapshots(projectId: string, sha?: string) {
		const resp = await invoke<Snapshot[]>('list_snapshots', {
			projectId: projectId,
			limit: 32,
			sha: sha
		});
		return resp;
	}
	async function getSnapshotDiff(projectId: string, sha: string) {
		const resp = await invoke<string>('snapshot_diff', {
			projectId: projectId,
			sha: sha
		});
		console.log(JSON.stringify(resp));
		return resp;
	}
	async function restoreSnapshot(projectId: string, sha: string) {
		await invoke<string>('restore_snapshot', {
			projectId: projectId,
			sha: sha
		});
		// TODO: is there a better way to update all the state?
		await goto(window.location.href, { replaceState: true });
	}
	function onLastInView() {
		if (!listElement) return;
		if (listElement.scrollTop + listElement.clientHeight >= listElement.scrollHeight) {
			listSnapshots(projectId, snapshots[snapshots.length - 1].id).then((rsp) => {
				snapshots = [...snapshots, ...rsp.slice(1)];
			});
		}
	}
	let listElement: HTMLElement | undefined = undefined;
	onMount(async () => {
		listSnapshots(projectId).then((rsp) => {
			snapshots = rsp;
		});
		if (listElement) listElement.addEventListener('scroll', onLastInView, true);
	});
	onDestroy(() => {
		listElement?.removeEventListener('scroll', onLastInView, true);
	});
</script>

<div class="container" bind:this={listElement}>
	{#each snapshots as entry, idx}
		<div class="card" style="margin-bottom: 4px;">
			<div class="entry" style="padding: 6px;">
				<div style="display: flex; align-items: center;">
					<div>id: {entry.id.slice(0, 7)}</div>
					<div style="flex-grow: 1;" />
					<div>
						{#if entry.linesAdded + entry.linesRemoved > 0}
							<Button
								style="pop"
								size="tag"
								icon="docs-filled"
								on:click={async () => await getSnapshotDiff(projectId, entry.id)}>diff</Button
							>
						{/if}
					</div>
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
						restored_from: {entry.details?.trailers
							.find((t) => t.key === 'restored_from')
							?.value?.slice(0, 7)}
					{:else if ['DeleteBranch', 'CreateBranch'].includes(entry.details?.operation || '')}
						name: {entry.details?.trailers.find((t) => t.key === 'name')?.value}
					{:else if ['ReorderBranches', 'UpdateBranchName', 'SelectDefaultVirtualBranch', 'UpdateBranchRemoteName'].includes(entry.details?.operation || '')}
						<div>
							before: {entry.details?.trailers.find((t) => t.key === 'before')?.value}
						</div>
						<div>
							after: {entry.details?.trailers.find((t) => t.key === 'after')?.value}
						</div>
					{:else if ['CreateCommit'].includes(entry.details?.operation || '')}
						message: {entry.details?.trailers.find((t) => t.key === 'message')?.value}
						sha: {entry.details?.trailers.find((t) => t.key === 'sha')?.value?.slice(0, 7)}
					{:else if ['UndoCommit', 'CreateCommit'].includes(entry.details?.operation || '')}
						sha: {entry.details?.trailers.find((t) => t.key === 'sha')?.value?.slice(0, 7)}
					{/if}
				</div>
				<div>
					lines added: {entry.linesAdded}
				</div>
				<div>
					lines removed: {entry.linesRemoved}
				</div>
				<div>
					<div>files changed:</div>
					{#each entry.filesChanged as filePath}
						<div style="padding-left: 16px;">{filePath}</div>
					{/each}
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

<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';

	type Props = {
		projectId: string;
		stackId: string;
		selectionId:
			| {
					type: 'commit';
					commitId: string;
			  }
			| {
					type: 'branch';
					stackId: string;
					branchName: string;
			  };
	};

	const { projectId, stackId, selectionId }: Props = $props();
	const [stackService] = inject(StackService);
	const changesResult = $derived(
		selectionId.type === 'commit'
			? stackService.commitChanges(projectId, selectionId.commitId)
			: stackService.branchChanges(projectId, selectionId.stackId, selectionId.branchName)
	);

	const headerTitle = $derived.by(() => {
		switch (selectionId.type) {
			case 'commit':
				return 'Changed files';
			case 'branch':
				return 'All changed files';
		}
	});

	let listMode: 'list' | 'tree' = $state('tree');

	let isHeaderSticky = $state(false);
</script>

{#if changesResult}
	<div class="changed-files">
		<ReduxResult result={changesResult.current}>
			{#snippet children(changes)}
				<div
					class="changed-files__header"
					class:sticky={isHeaderSticky}
					use:intersectionObserver={{
						callback: (entry) => {
							if (entry?.isIntersecting) {
								isHeaderSticky = false;
							} else {
								isHeaderSticky = true;
							}
						},
						options: {
							root: null,
							rootMargin: `-1px 0px 0px 0px`,
							threshold: 1
						}
					}}
				>
					<div class="changed-files__header-left">
						<h4 class="text-14 text-semibold">{headerTitle}</h4>
						<Badge>{changes.length}</Badge>
					</div>
					<FileListMode bind:mode={listMode} persist="committed" />
				</div>
				{#if changes.length > 0}
					<FileList {projectId} {stackId} {changes} {listMode} {selectionId} />
				{:else}
					<div class="text-12 text-body helper-text">(no changed files)</div>
				{/if}
			{/snippet}
		</ReduxResult>
	</div>
{:else}
	<p class="text-13 text-bold">Malformed props</p>
{/if}

<style>
	.changed-files {
		position: relative;
		display: flex;
		flex-direction: column;
	}

	.changed-files__header {
		z-index: var(--z-ground);
		position: sticky;
		top: -1px;
		padding: 10px 10px 10px 14px;
		display: flex;
		align-items: center;
		gap: 4px;
		justify-content: space-between;
		background-color: var(--clr-bg-1);

		&.sticky {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.changed-files__header-left {
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>

<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { inject } from '@gitbutler/core/context';
	import { FileListItem, Icon } from '@gitbutler/ui';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';

	type Props = {
		projectId: string;
		query: string;
		limit: number;
		onselect?: (filename: string) => void;
	};

	const { projectId, query, limit, onselect }: Props = $props();

	const fileService = inject(FILE_SERVICE);
	const files = $derived(fileService.findFiles(projectId, query, limit));

	let selected = $state(0);
	let dismissed = $state(false);

	$effect(() => {
		if (files) dismissed = false;
	});

	function onkeydown(e: KeyboardEvent) {
		if (!files.response || dismissed) return;
		const { key } = e;

		if (key === 'ArrowUp') {
			e.stopPropagation();
			selected = Math.max(selected - 1, 0);
		} else if (key === 'ArrowDown') {
			e.stopPropagation();
			selected = files ? Math.min(selected + 1, files.response.length - 1) : 0;
		} else if (key === 'Enter') {
			e.stopPropagation();
			e.preventDefault(); // Prevents newline in editor.
			const file = files.response[selected];
			if (file && onselect) {
				onselect(file);
			}
		} else if (key === 'Escape') {
			dismissed = true;
		}
	}
</script>

<svelte:body onkeydowncapture={onkeydown} />

{#if !dismissed}
	<div class="file-plugin" use:clickOutside={{ handler: () => (dismissed = true) }}>
		<ReduxResult {projectId} result={files.result}>
			{#snippet loading()}
				<div class="p-12">
					<Icon name="spinner" />
				</div>
			{/snippet}

			{#snippet children(files)}
				{#each files as file, i}
					<FileListItem
						filePath={file}
						onclick={() => onselect?.(file)}
						selected={i === selected}
						active={i === selected}
					/>
				{/each}
			{/snippet}
		</ReduxResult>
	</div>
{/if}

<style lang="postcss">
	.file-plugin {
		z-index: var(--z-lifted);
		position: absolute;
		bottom: calc(100% + 9px);
		max-width: 80%;
		overflow-x: hidden;
		overflow-y: auto;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}
</style>

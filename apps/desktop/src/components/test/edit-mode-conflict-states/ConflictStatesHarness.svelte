<script lang="ts">
	/**
	 * Test harness that mirrors the conflict-tracking $effect from
	 * EditCommitPanel: an effect re-runs refreshConflictStates whenever
	 * the watched response changes, exactly like the real component does.
	 */
	import { refreshConflictStates } from "$lib/files/conflictCheck";
	import { SvelteMap } from "svelte/reactivity";
	import type { ConflictState } from "$lib/files/conflictEntryPresence";
	import type { FileService } from "$lib/files/fileService";
	import type { ConflictEntryPresence } from "@gitbutler/but-sdk";

	type FileEntry = {
		path: string;
		conflictEntryPresence?: ConflictEntryPresence;
	};

	type Props = {
		files: FileEntry[];
		uncommittedResponse: unknown;
		fileService: FileService;
		projectId: string;
	};

	const { files, uncommittedResponse, fileService, projectId }: Props = $props();

	const conflictStates = new SvelteMap<string, ConflictState>();

	$effect(() => {
		void uncommittedResponse;
		refreshConflictStates(files, fileService, projectId, conflictStates);
	});
</script>

{#each files as file (file.path)}
	<div
		data-testid="file-{file.path}"
		data-conflict-state={conflictStates.get(file.path) ?? "unknown"}
	>
		{file.path}: {conflictStates.get(file.path) ?? "unknown"}
	</div>
{/each}

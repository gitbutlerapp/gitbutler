<script lang="ts">
	/**
	 * Test harness that mirrors the conflict-tracking $effect from
	 * EditCommitPanel. It reads conflicted files via fileService and
	 * maintains a reactive conflictStates map, exactly like the real
	 * component does.
	 */
	import { getConflictState } from "$lib/files/conflictEntryPresence";
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

		for (const file of files) {
			if (!file.conflictEntryPresence) continue;
			const presence = file.conflictEntryPresence;
			const path = file.path;
			fileService.readFromWorkspace(path, projectId).then((result) => {
				conflictStates.set(path, getConflictState(presence, result.data.content));
			});
		}
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

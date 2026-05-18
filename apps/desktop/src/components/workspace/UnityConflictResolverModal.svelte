<script lang="ts">
	import UnityConflictWorkbench from "$components/workspace/UnityConflictWorkbench.svelte";
	import { FILE_SERVICE } from "$lib/files/fileService";
	import { parseUnityConflictDocument } from "$lib/files/unityConflicts";
	import { inject } from "@gitbutler/core/context";
	import { Button, Modal } from "@gitbutler/ui";

	import type { UnityConflictDocument } from "$lib/files/unityConflicts";

	type Props = {
		projectId: string;
		onResolved?: (filePath: string) => void;
	};

	const { projectId, onResolved }: Props = $props();

	const fileService = inject(FILE_SERVICE);

	let modal = $state<Modal>();
	let filePath = $state("");
	let document = $state<UnityConflictDocument | null>(null);
	let errorMessage = $state<string | null>(null);
	let loading = $state(false);
	let applying = $state(false);

	export async function show(path: string) {
		filePath = path;
		errorMessage = null;
		document = null;
		loading = true;
		modal?.show();

		try {
			const result = await fileService.readFromWorkspace(path, projectId);
			const content = result.data.content ?? "";
			document = parseUnityConflictDocument(path, content);
			if (!document) {
				errorMessage =
					"GitButler can only render Unity conflict cards when the file contains Git conflict markers.";
			}
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : "Failed to load the conflicted Unity scene.";
		} finally {
			loading = false;
		}
	}

	async function handleApply(resolvedContent: string) {
		applying = true;
		try {
			await fileService.writeToWorkspace(filePath, projectId, resolvedContent);
			onResolved?.(filePath);
			await modal?.close();
		} finally {
			applying = false;
		}
	}
</script>

<Modal bind:this={modal} title="Resolve Unity scene conflicts" width={960} noPadding>
	{#snippet children(_, close)}
		<div class="unity-modal">
			{#if loading}
				<p class="text-13 text-body unity-modal__state">Loading conflicted scene…</p>
			{:else if errorMessage}
				<div class="unity-modal__state">
					<p class="text-13 text-body">{errorMessage}</p>
					<div class="unity-modal__actions">
						<Button kind="outline" onclick={close}>Close</Button>
					</div>
				</div>
			{:else if document}
				<div class="unity-modal__content">
					<UnityConflictWorkbench {filePath} {document} {applying} onApply={handleApply} />
				</div>
			{/if}
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.unity-modal {
		max-height: 80vh;
		padding: 16px;
		overflow: auto;
	}

	.unity-modal__state {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 16px;
	}

	.unity-modal__actions {
		display: flex;
		justify-content: flex-end;
	}

	.unity-modal__content {
		padding-right: 4px;
	}
</style>

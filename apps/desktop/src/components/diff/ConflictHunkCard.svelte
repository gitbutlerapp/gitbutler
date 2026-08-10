<script lang="ts" module>
	// Every apply rewrites the commit, so a resolution started from another
	// card would target an id that no longer exists. Reactive and module-wide so
	// every card's buttons disable while one resolution is in flight.
	let anyCardResolving = $state(false);
</script>

<script lang="ts">
	import { projectAiGenEnabled } from "$lib/config/config";
	import { showToast } from "$lib/notifications/toasts";
	import { maybeGetStackContext } from "$lib/stacks/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, Icon } from "@gitbutler/ui";
	import type { ConflictHunk, HunkResolution } from "@gitbutler/but-sdk";

	type Props = {
		hunk: ConflictHunk;
		index: number;
		total: number;
		projectId: string;
		stackId?: string;
		/// The conflicted commit; without it the card is view-only.
		commitId?: string;
		path?: string;
	};

	const { hunk, index, total, projectId, stackId, commitId, path }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const controller = maybeGetStackContext();

	const [resolveHunks] = stackService.resolveCommitConflictHunks;
	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	let pending = $state<"ours" | "theirs" | "content" | "ai">();
	let editing = $state(false);
	let draft = $state("");

	// This card is resolving, or another one is — either way, disable actions.
	const busy = $derived(!!pending || anyCardResolving);

	async function applyResolution(resolution: HunkResolution) {
		if (!commitId || !path || busy) return;
		anyCardResolving = true;
		pending = resolution.type;
		try {
			const result = await resolveHunks({
				projectId,
				stackId,
				commitId,
				specs: [{ path, hunk: index + 1, resolution }],
			});
			editing = false;
			// The apply rewrites the commit; follow it so the remaining
			// conflicts stay in view and can be resolved one after another.
			const selection = controller?.selection.current;
			if (controller && selection?.commitId === commitId) {
				controller.selection.set({
					branchName: selection.branchName,
					commitId: result.newCommit,
					upstream: false,
					previewOpen: selection.previewOpen ?? true,
				});
			}
			if (result.remaining.length === 0) {
				showToast({
					style: "success",
					title: "All conflicts resolved",
					message: result.commitEmptied
						? "The commit is now empty: the kept content matches its parent, so this commit no longer changes anything. Undo from the operations history if that wasn't the intent."
						: "The commit is no longer conflicted. Undo from the operations history.",
				});
			}
		} catch (error: unknown) {
			showToast({
				style: "danger",
				title: "Failed to resolve the conflict",
				error,
			});
		} finally {
			anyCardResolving = false;
			pending = undefined;
		}
	}

	function startEditing() {
		draft = hunk.theirs;
		editing = true;
	}
</script>

<div class="conflict-card">
	<div class="conflict-card__title">
		<Icon name="warning" color="var(--fill-warn-bg)" />
		<span class="text-12 text-semibold">
			Unresolved conflict{total > 1 ? ` ${index + 1} of ${total}` : ""}
		</span>
	</div>
	<div class="conflict-card__section ours">
		<div class="text-11 text-semibold conflict-card__label">Current base</div>
		<pre class="text-12 conflict-card__content">{hunk.ours}</pre>
	</div>
	{#if hunk.base !== null}
		<div class="conflict-card__section">
			<div class="text-11 text-semibold conflict-card__label">Common ancestor</div>
			<pre class="text-12 conflict-card__content">{hunk.base}</pre>
		</div>
	{/if}
	<div class="conflict-card__section theirs">
		<div class="text-11 text-semibold conflict-card__label">This commit</div>
		<pre class="text-12 conflict-card__content">{hunk.theirs}</pre>
	</div>
	{#if commitId && path}
		{#if editing}
			<div class="conflict-card__editor">
				<div class="text-11 conflict-card__editor-hint">
					Edit the merged result that replaces this conflict. Leaving it empty deletes the region.
				</div>
				<!-- svelte-ignore a11y_autofocus -->
				<textarea
					class="text-12 conflict-card__textarea"
					bind:value={draft}
					rows={Math.min(Math.max(draft.split("\n").length + 1, 3), 16)}
					spellcheck="false"
					autofocus
					disabled={busy}
				></textarea>
				<div class="conflict-card__actions">
					<Button
						kind="solid"
						style="pop"
						size="tag"
						loading={pending === "content"}
						disabled={busy}
						onclick={() => applyResolution({ type: "content", subject: draft })}
					>
						Apply resolution
					</Button>
					<Button kind="outline" size="tag" disabled={busy} onclick={() => (editing = false)}>
						Cancel
					</Button>
				</div>
			</div>
		{:else}
			<div class="conflict-card__actions">
				<Button
					kind="outline"
					size="tag"
					loading={pending === "ours"}
					disabled={busy}
					onclick={() => applyResolution({ type: "ours" })}
				>
					Use current base
				</Button>
				<Button
					kind="outline"
					size="tag"
					loading={pending === "theirs"}
					disabled={busy}
					onclick={() => applyResolution({ type: "theirs" })}
				>
					Use this commit
				</Button>
				<Button kind="outline" size="tag" disabled={busy} onclick={startEditing}>Edit…</Button>
				{#if $aiGenEnabled}
					<Button
						kind="outline"
						size="tag"
						icon="ai"
						loading={pending === "ai"}
						disabled={busy}
						onclick={() => applyResolution({ type: "ai" })}
					>
						Resolve with AI
					</Button>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style>
	.conflict-card {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
	}

	.conflict-card__title {
		display: flex;
		align-items: center;
		padding: 6px 10px;
		gap: 6px;
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-warn);
		color: var(--text-warn);
	}

	.conflict-card__section {
		display: flex;
		flex-direction: column;

		&:not(:last-child) {
			border-bottom: 1px solid var(--border-2);
		}

		&.ours .conflict-card__content {
			background-color: var(--diff-deletion-line-bg);
		}

		&.theirs .conflict-card__content {
			background-color: var(--diff-addition-line-bg);
		}
	}

	.conflict-card__label {
		padding: 4px 10px;
		background-color: var(--bg-2);
		color: var(--text-3);
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.conflict-card__content {
		margin: 0;
		padding: 6px 10px;
		overflow-x: auto;
		font-size: var(--diff-font-size, 12px);
		font-family: var(--font-mono);
		white-space: pre;
	}

	.conflict-card__actions {
		display: flex;
		flex-wrap: wrap;
		padding: 8px 10px;
		gap: 6px;
		border-top: 1px solid var(--border-2);
		background-color: var(--bg-2);
	}

	.conflict-card__editor {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--border-2);
	}

	.conflict-card__editor-hint {
		padding: 6px 10px;
		color: var(--text-2);
	}

	.conflict-card__editor .conflict-card__actions {
		border-top: none;
	}

	.conflict-card__textarea {
		margin: 0 10px;
		padding: 6px 8px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-s);
		background-color: var(--bg-1);
		color: var(--text-1);
		font-family: var(--font-mono);
		resize: vertical;
	}
</style>

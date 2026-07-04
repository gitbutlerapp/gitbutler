<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Modal, Button } from "@gitbutler/ui";

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	let modal: Modal | undefined = $state();
	let commitId: string | undefined = $state();

	export function show(forCommitId: string) {
		commitId = forCommitId;
		modal?.show();
	}

	const conflictsQuery = $derived(
		commitId ? stackService.commitConflicts(projectId, commitId) : undefined,
	);
</script>

<Modal bind:this={modal} width={600} title="Conflicts in this commit">
	{#if conflictsQuery}
		<ReduxResult {projectId} result={conflictsQuery.result}>
			{#snippet children(conflicts)}
				<div class="conflicts">
					{#each conflicts.files as file (file.path)}
						<div class="file">
							<h3 class="text-13 text-semibold">{file.path}</h3>
							{#each file.hunks as hunk, index}
								<div class="hunk">
									<div class="text-11 text-semibold hunk__title">
										Conflict {index + 1} of {file.hunks.length}
									</div>
									<div class="text-11 hunk__label">ours — the new base</div>
									<pre class="text-12 hunk__content">{hunk.ours}</pre>
									{#if hunk.base !== null}
										<div class="text-11 hunk__label">base — common ancestor</div>
										<pre class="text-12 hunk__content">{hunk.base}</pre>
									{/if}
									<div class="text-11 hunk__label">theirs — this commit</div>
									<pre class="text-12 hunk__content">{hunk.theirs}</pre>
								</div>
							{/each}
						</div>
					{/each}
				</div>
			{/snippet}
		</ReduxResult>
	{/if}

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Close</Button>
	{/snippet}
</Modal>

<style>
	.conflicts {
		display: flex;
		flex-direction: column;
		max-height: 60vh;
		overflow-y: auto;
		gap: 16px;
	}

	.file {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.hunk {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.hunk__title {
		padding: 6px 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.hunk__label {
		padding: 4px 8px;
		color: var(--clr-text-2);
		background-color: var(--clr-bg-2);
	}

	.hunk__content {
		margin: 0;
		padding: 4px 8px;
		overflow-x: auto;
		font-family: var(--fontfamily-mono);
		white-space: pre;
	}
</style>

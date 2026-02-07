<script lang="ts">
	import FileList from "$components/FileList.svelte";
	import UnifiedDiffView from "$components/UnifiedDiffView.svelte";
	import { createCommitSelection } from "$lib/selection/key";
	import { inject } from "@gitbutler/core/context";
	import type { UnifiedDiff } from "$lib/hunks/diff";
	import type { SharedCommitPayload } from "$lib/irc/sharedStack";

	type Props = {
		projectId: string;
		commit: SharedCommitPayload;
	};

	const { projectId, commit }: Props = $props();

	const selectionId = createCommitSelection({ commitId: commit.commitId });
	const changes = $derived(commit.commit.files.map((f) => f.change));

	let selectedIndex: number = $state(0);

	const selectedFile = $derived(
		selectedIndex !== undefined ? commit.commit.files[selectedIndex] : undefined,
	);
	const selectedChange = $derived(selectedFile?.change);
	const selectedDiff: UnifiedDiff | null = $derived(
		selectedFile
			? {
					type: "Patch" as const,
					subject: {
						hunks: selectedFile.hunks,
						isResultOfBinaryToTextConversion: false,
						linesAdded: 0,
						linesRemoved: 0,
					},
				}
			: null,
	);
</script>

<div class="irc-commit">
	<div class="irc-commit-header text-12 text-semibold">
		{commit.commit.message.split("\n")[0]}
	</div>
	<div class="irc-commit-body">
		<div class="irc-commit-files">
			<FileList
				{projectId}
				{changes}
				listMode="list"
				{selectionId}
				onFileClick={(index) => (selectedIndex = index)}
			/>
		</div>
		{#if selectedChange && selectedDiff}
			<div class="irc-commit-diff">
				<UnifiedDiffView
					{projectId}
					selectable={false}
					change={selectedChange}
					diff={selectedDiff}
					topPadding
					{selectionId}
				/>
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.irc-commit {
		display: flex;
		flex-direction: column;
		max-width: 100%;
		max-height: 300px;
		margin-bottom: 6px;
		overflow: hidden;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}
	.irc-commit-header {
		padding: 6px 12px;
		border-bottom: 1px solid var(--clr-border-3);
		background-color: var(--clr-bg-2);
	}
	.irc-commit-body {
		display: flex;
		flex-grow: 1;
		overflow: hidden;
	}
	.irc-commit-files {
		flex-shrink: 0;
		width: 30%;
		border-right: 1px solid var(--clr-border-3);
	}
	.irc-commit-diff {
		flex-grow: 1;
		overflow: auto;
	}
</style>

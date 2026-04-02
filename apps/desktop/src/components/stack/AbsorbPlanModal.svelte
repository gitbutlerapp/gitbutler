<script lang="ts">
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		Button,
		CopyButton,
		FileListItem,
		Icon,
		Modal,
		ModalHeader,
		ScrollableContainer,
		TestId,
		chipToasts,
	} from "@gitbutler/ui";
	import { tick } from "svelte";
	import type { CommitAbsorption, FileAbsorption, TreeChange } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId: string | undefined;
	};

	const { projectId, stackId }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const [absorb, absorbingChanges] = stackService.absorb;

	let modal = $state<ReturnType<typeof Modal> | undefined>();
	let absorbPlan = $state<CommitAbsorption[]>([]);
	let isScrollVisible = $state(true);

	export async function show(changes: TreeChange[]) {
		const changesToAbsorb = $state.snapshot(changes);
		const plan = await stackService.fetchAbsorbPlan(projectId, {
			type: "treeChanges",
			subject: {
				changes: changesToAbsorb,
				assigned_stack_id: stackId ?? null,
			},
		});
		if (!plan || plan.length === 0) {
			chipToasts.error("No suitable commits found to absorb changes into.");
			return;
		}
		absorbPlan = plan;
		await tick();
		modal?.show(null);
	}

	function uniquePaths(files: FileAbsorption[]): string[] {
		const pathSet = new Set<string>();
		for (const file of files) {
			pathSet.add(file.path);
		}
		return Array.from(pathSet);
	}
</script>

<Modal
	width={500}
	noPadding
	bind:this={modal}
	testId={TestId.AbsobModal}
	onSubmit={async () => {
		try {
			await chipToasts.promise(absorb({ projectId, absorptionPlan: absorbPlan }), {
				loading: "Absorbing changes",
				success: "Changes absorbed successfully",
				error: "Failed to absorb changes",
			});
			modal?.close();
		} catch (error) {
			console.error("Failed to absorb changes:", error);
		}
	}}
>
	<ModalHeader sticky={!isScrollVisible}>Absorb Changes into Commits</ModalHeader>
	<ScrollableContainer onscrollTop={(visible) => (isScrollVisible = visible)}>
		<div class="absorb-plan-content">
			<p class="text-13 text-body clr-text-2">
				The following changes will be absorbed into their respective commits:
			</p>
			<div class="commit-absorptions">
				{#each absorbPlan as commitAbsorption}
					{@const uniqueFilePaths = uniquePaths(commitAbsorption.files)}
					<div class="commit-absorption" data-testid={TestId.AbsorbModal_CommitAbsorption}>
						{#if commitAbsorption.reason !== "default_stack"}
							<div class="absorption__reason text-12 text-body clr-text-2">
								{#if commitAbsorption.reason === "hunk_dependency"}
									📍 Files depend on the commit due to overlapping hunks
								{:else if commitAbsorption.reason === "stack_assignment"}
									🔖 Files assigned to this stack
								{/if}
							</div>
						{/if}

						<div class="absorption__content">
							<div class="commit-header">
								<Icon name="commit" />

								<div class="flex gap-8 overflow-hidden align-center full-width">
									<p class="text-13 text-semibold truncate flex-1">
										{commitAbsorption.commitSummary.split("\n")[0]}
									</p>
									<CopyButton
										class="text-12 clr-text-2"
										text={commitAbsorption.commitId}
										onclick={() => {
											clipboardService.write(commitAbsorption.commitId, {
												message: "Commit ID copied",
											});
										}}
									/>
								</div>
							</div>

							<ul class="file-list">
								{#each uniqueFilePaths as filePath (filePath)}
									<FileListItem
										{filePath}
										clickable={false}
										listMode="list"
										isLast={uniqueFilePaths.indexOf(filePath) === uniqueFilePaths.length - 1}
									/>
								{/each}
							</ul>
						</div>
					</div>
				{/each}
			</div>
		</div>
	</ScrollableContainer>

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button
			style="pop"
			type="submit"
			loading={absorbingChanges.current.isLoading}
			disabled={absorbPlan.length === 0 || absorbingChanges.current.isLoading}
			testId={TestId.AbsorbModal_ActionButton}
		>
			Absorb changes
		</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.absorb-plan-content {
		display: flex;
		flex-direction: column;
		padding: 16px;
		padding-top: 0;
		gap: 12px;
	}
	.commit-absorptions {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.commit-absorption {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background-color: var(--bg-1);
	}
	.absorption__reason {
		display: flex;
		padding: 8px;
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-2);
	}
	.absorption__content {
		display: flex;
		flex-direction: column;
		padding: 12px;
	}
	.commit-header {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.file-list {
		display: flex;
		flex-direction: column;
		margin-top: 12px;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
	}
</style>

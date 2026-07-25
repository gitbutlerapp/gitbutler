<script lang="ts">
	import { goto } from "$app/navigation";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { showError } from "$lib/error/showError";
	import { workspacePath } from "$lib/routes/routes.svelte";
	import { toCommitMovePlacement } from "$lib/stacks/commitMovePlacement";
	import { cherryPickTargets } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup, InfoMessage, Modal, RadioButton } from "@gitbutler/ui";

	type Props = {
		projectId: string;
		/** The commit hash to cherry-pick */
		subject?: string;
	};

	let { projectId, subject }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	let modalRef = $state<Modal>();

	const stacksResult = $derived(stackService.stacks(projectId));

	let selectedBranchName = $state<string | undefined>(undefined);
	const [cherryPick, cherryPickResult] = stackService.commitCherryPick;

	export function close() {
		modalRef?.close();
	}

	export function open() {
		// The modal instance is reused for every commit, so a selection left over from a previous
		// open could name a stack that is no longer applied.
		selectedBranchName = undefined;
		modalRef?.show();
	}

	async function handleApply() {
		if (!selectedBranchName || !subject) return;

		// Cherry-picks take the same graph placement as moves, so the target is
		// expressed the same way: the tip of the destination stack.
		const { relativeTo, side } = toCommitMovePlacement({
			targetBranchName: selectedBranchName,
			targetCommitId: "top",
		});

		try {
			await cherryPick({
				projectId,
				sourceCommitIds: [subject],
				relativeTo,
				side,
				dryRun: false,
			});
		} catch (error) {
			// Keep the modal open so another stack can be tried.
			showError("Cannot cherry-pick commit", error);
			return;
		}

		goto(workspacePath(projectId));

		close();
	}

	const isApplying = $derived(cherryPickResult.current.isLoading);

	/** A stack whose top segment lost its branch name has nothing to place the copy against. */
	function targetsMessage(targetCount: number, stackCount: number): string {
		if (targetCount > 0) return "Select the stack to copy this commit into.";
		if (stackCount > 0) return "No applied stack has a named branch to copy this commit into.";
		return "No stacks are currently applied to the workspace.";
	}

	function handleStackSelectionChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		const selected = formData.get("stackSelection") as string | null;
		if (selected) {
			selectedBranchName = selected;
		}
	}
</script>

<Modal bind:this={modalRef} title="Cherry-pick commit" width={500}>
	<ReduxResult {projectId} result={stacksResult.result}>
		{#snippet children(stacks)}
			{@const targets = cherryPickTargets(stacks)}
			<div class="cherry-apply-modal">
				<InfoMessage style="info" outlined>
					{#snippet content()}
						{targetsMessage(targets.length, stacks.length)}
					{/snippet}
				</InfoMessage>

				{#if targets.length > 0}
					<CardGroup>
						<form onchange={(e) => handleStackSelectionChange(e.currentTarget)}>
							{#each targets as { branchName, branchCount }}
								<CardGroup.Item labelFor="stack-{branchName}">
									{#snippet title()}
										{branchName}
									{/snippet}
									{#snippet caption()}
										{branchCount}
										{branchCount === 1 ? "branch" : "branches"}
									{/snippet}
									{#snippet actions()}
										<RadioButton
											name="stackSelection"
											value={branchName}
											id="stack-{branchName}"
											checked={selectedBranchName === branchName}
										/>
									{/snippet}
								</CardGroup.Item>
							{/each}
						</form>
					</CardGroup>
				{/if}
			</div>
		{/snippet}
	</ReduxResult>
	{#snippet controls()}
		<Button kind="outline" onclick={close} disabled={isApplying}>Cancel</Button>
		<Button
			style="pop"
			onclick={handleApply}
			disabled={!selectedBranchName || isApplying}
			loading={isApplying}
		>
			Cherry-pick
		</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.cherry-apply-modal {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>

<script lang="ts">
	import BranchNameTextbox from "$components/branch/BranchNameTextbox.svelte";
	import { changesToDiffSpec } from "$lib/commits/utils";
	import { autoSelectBranchCreationFeature } from "$lib/config/uiFeatureFlags";
	import { isTreeChange, type TreeChange } from "$lib/hunks/change";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { AsyncButton, Button, Modal } from "@gitbutler/ui";

	type ChangedFilesItem = {
		changes: TreeChange[];
	};

	function isChangedFilesItem(item: unknown): item is ChangedFilesItem {
		return (
			typeof item === "object" &&
			item !== null &&
			"changes" in item &&
			Array.isArray(item.changes) &&
			item.changes.every(isTreeChange)
		);
	}

	type ChangedFolderItem = ChangedFilesItem & { path: string };

	function isChangedFolderItem(item: ChangedFilesItem): item is ChangedFolderItem {
		return "path" in item && typeof item.path === "string";
	}

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	let modal: ReturnType<typeof Modal> | undefined;
	let stashBranchName = $state<string>();
	let slugifiedRefName: string | undefined = $state();
	let stashBranchNameInput = $state<ReturnType<typeof BranchNameTextbox>>();

	export async function show(item: ChangedFilesItem) {
		slugifiedRefName = undefined;
		modal?.show(item);
		stashBranchName = await stackService.fetchNewBranchName(projectId);
		if ($autoSelectBranchCreationFeature) {
			await stashBranchNameInput?.selectAll();
		}
	}

	async function confirmStashIntoBranch(item: ChangedFilesItem, branchName: string | undefined) {
		if (!branchName) return;

		await stackService.stashIntoBranch({
			projectId,
			branchName,
			worktreeChanges: changesToDiffSpec(item.changes),
		});

		modal?.close();
	}
</script>

<Modal width={434} type="info" title="Stash changes into a new branch" bind:this={modal}>
	{#snippet children(item)}
		<div class="content-wrap">
			<BranchNameTextbox
				bind:this={stashBranchNameInput}
				id="stashBranchName"
				placeholder="Enter your branch name..."
				bind:value={stashBranchName}
				autofocus
				onslugifiedvalue={(value) => (slugifiedRefName = value)}
			/>
			<div class="explanation">
				<p class="primary-text">
					{#if isChangedFilesItem(item) && isChangedFolderItem(item)}
						All changes in this folder
					{:else}
						Your selected changes
					{/if}
					will be moved to a new branch and removed from your current workspace. To get these changes
					back later, switch to the new branch and uncommit the stash.
				</p>
			</div>

			<div class="technical-note">
				<p class="text-12 text-body clr-text-2">
					💡 This creates a new branch, commits your changes, then unapplies the branch. Future
					versions will have simpler stash management.
				</p>
			</div>
		</div>
	{/snippet}
	{#snippet controls(close, item)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<AsyncButton
			style="pop"
			disabled={!slugifiedRefName}
			type="submit"
			action={async () => {
				if (isChangedFilesItem(item)) await confirmStashIntoBranch(item, slugifiedRefName);
			}}
		>
			Stash into branch
		</AsyncButton>
	{/snippet}
</Modal>

<style lang="postcss">
	.content-wrap {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>

<script lang="ts">
	import BranchNameTextbox from "$components/branch/BranchNameTextbox.svelte";
	import { changesToDiffSpec } from "$lib/commits/utils";
	import { autoSelectBranchCreationFeature } from "$lib/config/uiFeatureFlags";
	import { isTreeChange } from "$lib/hunks/change";
	import { t } from "$lib/i18n";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { AsyncButton, Button, Modal } from "@gitbutler/ui-svelte";
	import type { TreeChange } from "@gitbutler/but-sdk";

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
	let normalizedRefName: string | undefined = $state();
	let isStashBranchNameValid = $state(false);
	let stashBranchNameInput = $state<ReturnType<typeof BranchNameTextbox>>();

	export async function show(item: ChangedFilesItem) {
		normalizedRefName = undefined;
		isStashBranchNameValid = false;
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
				placeholder={t("enter-your-branch-name")}
				bind:value={stashBranchName}
				autofocus
				onnormalizedvalue={(value) => (normalizedRefName = value)}
				onvalidationchange={(isValid) => (isStashBranchNameValid = isValid)}
			/>
			<div class="explanation">
				<p class="primary-text">
					{#if isChangedFilesItem(item) && isChangedFolderItem(item)}
						{t("all-changes-in-this-folder")}
					{:else}
						{t("your-selected-changes")}
					{/if}
					{t("will-be-moved-to-a-new-branch-and-removed-from-your-current-")}
				</p>
			</div>

			<div class="technical-note">
				<p class="text-12 text-body clr-text-2">
					{t("this-creates-a-new-branch-commits-your-changes-then-unapplie")}
				</p>
			</div>
		</div>
	{/snippet}
	{#snippet controls(close, item)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<AsyncButton
			style="pop"
			disabled={!isStashBranchNameValid}
			type="submit"
			action={async () => {
				if (isChangedFilesItem(item)) await confirmStashIntoBranch(item, normalizedRefName);
			}}
		>
			{t("stash-into-branch-2")}
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

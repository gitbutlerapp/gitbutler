<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import SettingsSection from "$components/shared/SettingsSection.svelte";
	import RemoveWorktreeModal from "$components/stack/RemoveWorktreeModal.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup } from "@gitbutler/ui-svelte";
	import type { ListedWorktree } from "@gitbutler/but-sdk";

	const { projectId }: { projectId: string } = $props();

	const stackService = inject(STACK_SERVICE);
	const worktreesQuery = $derived(stackService.worktreesList(projectId));

	let removing = $state<string>();
	let removeWorktreeModal = $state<RemoveWorktreeModal>();

	function label(worktree: ListedWorktree) {
		return worktree.refName?.replace(/^refs\/heads\//, "") ?? `${worktree.name} (detached)`;
	}
</script>

{#snippet rows(worktrees: ListedWorktree[], archived: boolean)}
	<CardGroup>
		{#each worktrees as worktree (worktree.name)}
			<CardGroup.Item alignment="center">
				{#snippet title()}
					{label(worktree)}
				{/snippet}
				{#snippet caption()}
					{worktree.path}
				{/snippet}
				{#snippet actions()}
					<Button
						kind="outline"
						onclick={async () =>
							await stackService.setWorktreeArchived({
								projectId,
								name: worktree.name,
								archived: !archived,
							})}
					>
						{archived ? "Unarchive" : "Archive"}
					</Button>
					<Button
						kind="ghost"
						icon="bin"
						tooltip="Remove worktree…"
						onclick={() => {
							removing = worktree.name;
							removeWorktreeModal?.show();
						}}
					/>
				{/snippet}
			</CardGroup.Item>
		{/each}
	</CardGroup>
{/snippet}

<SettingsSection>
	<ReduxResult {projectId} result={worktreesQuery.result}>
		{#snippet children(listing)}
			{#if listing.active.length === 0}
				<p class="text-13 text-body">
					No active worktrees. A worktree added from now on shows up here and in the workspace.
				</p>
			{:else}
				{@render rows(listing.active, false)}
			{/if}
			{#if listing.archived.length > 0}
				<h2 class="text-15 text-bold">Archived</h2>
				{@render rows(listing.archived, true)}
			{/if}
		{/snippet}
	</ReduxResult>
</SettingsSection>

<RemoveWorktreeModal bind:this={removeWorktreeModal} {projectId} name={removing ?? ""} />

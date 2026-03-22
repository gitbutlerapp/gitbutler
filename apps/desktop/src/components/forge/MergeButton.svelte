<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { MergeMethod } from "$lib/forge/interface/types";
	import { showError } from "$lib/notifications/toasts";
	import { inject } from "@gitbutler/core/context";
	import { persisted, type Persisted } from "@gitbutler/shared/persisted";

	import { ContextMenuItem, ContextMenuSection, DropdownButton } from "@gitbutler/ui";
	import type { ButtonProps } from "@gitbutler/ui";

	interface Props {
		projectId: string;
		onclick: (method: MergeMethod) => Promise<void>;
		disabled?: boolean;
		wide?: boolean;
		tooltip?: string;
		style?: ButtonProps["style"];
		kind?: ButtonProps["kind"];
		isDraft?: boolean;
		onSetDraft?: (draft: boolean) => Promise<void>;
	}

	const {
		projectId,
		onclick,
		disabled = false,
		wide = false,
		tooltip = "",
		style = "gray",
		kind = "outline",
		isDraft = false,
		onSetDraft,
	}: Props = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const isGitLab = $derived(forge.current.name === "gitlab");

	function persistedAction(projectId: string): Persisted<MergeMethod> {
		const key = "projectMergeMethod";
		return persisted<MergeMethod>(MergeMethod.Merge, key + projectId);
	}

	const action = persistedAction(projectId);

	// If GitLab and action is rebase, reset to merge
	$effect(() => {
		if (isGitLab && $action === MergeMethod.Rebase) {
			$action = MergeMethod.Merge;
		}
	});

	let dropDown: ReturnType<typeof DropdownButton> | undefined;
	let loading = $state(false);

	// Available merge methods based on forge type
	const availableMethods = $derived(
		isGitLab
			? [MergeMethod.Merge, MergeMethod.Squash]
			: [MergeMethod.Merge, MergeMethod.Rebase, MergeMethod.Squash],
	);

	const labels = $derived({
		[MergeMethod.Merge]: "Merge",
		[MergeMethod.Rebase]: "Rebase and merge",
		[MergeMethod.Squash]: "Squash and merge",
	});
</script>

<DropdownButton
	bind:this={dropDown}
	onclick={async () => {
		loading = true;
		try {
			await onclick?.($action);
		} finally {
			loading = false;
		}
	}}
	{style}
	{kind}
	{loading}
	{wide}
	{tooltip}
	{disabled}
>
	{labels[$action]}
	{#snippet contextMenuSlot()}
		<ContextMenuSection>
			{#each availableMethods as method}
				<ContextMenuItem
					label={labels[method]}
					onclick={() => {
						$action = method;
						dropDown?.close();
					}}
				/>
			{/each}
		</ContextMenuSection>
		{#if onSetDraft}
			<ContextMenuSection>
				<ContextMenuItem
					label={isDraft ? "Ready for review" : "Convert to draft"}
					onclick={async () => {
						dropDown?.close();
						loading = true;
						try {
							await onSetDraft(!isDraft);
						} catch (err: unknown) {
							showError("Failed to update draft status", err);
						} finally {
							loading = false;
						}
					}}
				/>
			</ContextMenuSection>
		{/if}
	{/snippet}
</DropdownButton>

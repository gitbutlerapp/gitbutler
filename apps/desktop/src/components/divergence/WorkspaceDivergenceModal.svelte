<script lang="ts">
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		AsyncButton,
		Modal,
		Select,
		SelectItem,
		Icon,
		InfoMessage,
		type SelectItemType,
	} from "@gitbutler/ui";
	import { SvelteMap } from "svelte/reactivity";
	import type {
		DivergenceResolution,
		DivergenceStatuses,
		DivergenceApproach,
		StackRefDivergence,
	} from "@gitbutler/but-sdk";

	interface Props {
		projectId: string;
		onResolved?: () => void;
		/** Optional custom resolve handler. When set, replaces the default resolveWorkspaceDivergence call. */
		onResolve?: (resolutions: DivergenceResolution[]) => Promise<void>;
	}

	const { projectId, onResolved, onResolve }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const [resolveWorkspaceDivergence, resolveMutation] =
		baseBranchService.resolveWorkspaceDivergence;

	let modal = $state<Modal>();
	let divergences = $state<StackRefDivergence[]>([]);
	let resolveError = $state<string | undefined>();

	const resolutionChoices = new SvelteMap<string, DivergenceApproach>();

	function defaultApproach(divergence: StackRefDivergence): DivergenceApproach {
		switch (divergence.status.type) {
			case "movedToSameTree":
				return { type: "includeAsIs" };
			case "movedAboveBase":
				return divergence.status.conflicted ? { type: "exclude" } : { type: "includeAsIs" };
			case "movedBelowBase":
			case "deleted":
				return { type: "exclude" };
		}
	}

	function statusLabel(status: StackRefDivergence["status"]): string {
		switch (status.type) {
			case "deleted":
				return "Deleted";
			case "movedAboveBase":
				return status.conflicted ? "Moved (conflicts)" : "Moved";
			case "movedBelowBase":
				return "Moved below base";
			case "movedToSameTree":
				return "Moved (same content)";
		}
	}

	function statusDescription(status: StackRefDivergence["status"]): string {
		switch (status.type) {
			case "deleted":
				return "This branch ref was deleted outside of GitButler.";
			case "movedAboveBase":
				return status.conflicted
					? "This branch was moved and conflicts with the current workspace."
					: "This branch was moved to a different commit. Including it will change your workspace files.";
			case "movedBelowBase":
				return "This branch was moved to a commit below the target base. It has no commits in the workspace.";
			case "movedToSameTree":
				return "This branch was moved but the file content is identical. Including it won't change your workspace.";
		}
	}

	function optionsForDivergence(divergence: StackRefDivergence): SelectItemType<string>[] {
		const options: SelectItemType<string>[] = [{ label: "Include as-is", value: "includeAsIs" }];
		if (divergence.status.type !== "deleted" && divergence.status.type !== "movedBelowBase") {
			options.push({ label: "Include (rebase)", value: "includeRebase" });
		}
		options.push({ label: "Exclude", value: "exclude" });
		return options;
	}

	export function show(statuses: DivergenceStatuses) {
		if (statuses.type !== "divergedRefs") return;
		showDivergences(statuses.subject.divergences);
	}

	export function showDivergences(items: StackRefDivergence[]) {
		resolveError = undefined;
		resolutionChoices.clear();
		divergences = items;

		for (const d of divergences) {
			resolutionChoices.set(d.stackId, defaultApproach(d));
		}

		modal?.show();
	}

	async function resolve(close: () => void) {
		resolveError = undefined;

		const resolutions: DivergenceResolution[] = divergences.map((d) => ({
			stackId: d.stackId,
			approach: resolutionChoices.get(d.stackId) ?? defaultApproach(d),
		}));

		try {
			if (onResolve) {
				await onResolve(resolutions);
			} else {
				await resolveWorkspaceDivergence({ projectId, resolutions });
			}
			close();
			onResolved?.();
		} catch (error: unknown) {
			resolveError = error instanceof Error ? error.message : String(error);
		}
	}

	const anyIncluded = $derived(
		divergences.some((d) => {
			const choice = resolutionChoices.get(d.stackId);
			return choice?.type === "includeAsIs" || choice?.type === "includeRebase";
		}),
	);
</script>

<Modal bind:this={modal} width={520}>
	{#snippet children(_item, _close)}
		<div class="divergence-modal" data-testid="workspace-divergence-modal">
			<div class="divergence-modal__header">
				<Icon name="warning" color="warning" />
				<h3 class="text-15 text-body text-bold">Branch refs changed externally</h3>
			</div>

			<p class="divergence-modal__description text-13 text-body">
				One or more branch refs in your workspace were modified outside of GitButler. Choose how to
				handle each branch. Including a changed branch may cause a workspace checkout.
			</p>

			<div class="divergence-modal__list">
				{#each divergences as divergence (divergence.stackId)}
					{@const choice = resolutionChoices.get(divergence.stackId) ?? defaultApproach(divergence)}
					<div class="divergence-item" data-testid="workspace-divergence-modal-divergence-item">
						<div class="divergence-item__info">
							<p class="text-13 text-body text-bold">{divergence.refName || "unknown"}</p>
							<p class="text-12 text-body clr-text-2">{statusLabel(divergence.status)}</p>
							<p class="text-12 text-body clr-text-3">
								{statusDescription(divergence.status)}
							</p>
						</div>
						<div class="divergence-item__action">
							<Select
								value={choice.type}
								maxWidth={150}
								options={optionsForDivergence(divergence)}
								onselect={(value) => {
									resolutionChoices.set(divergence.stackId, {
										type: value,
									} as DivergenceApproach);
								}}
							>
								{#snippet itemSnippet({ item, highlighted })}
									<SelectItem selected={highlighted} {highlighted}>
										{item.label}
									</SelectItem>
								{/snippet}
							</Select>
						</div>
					</div>
				{/each}
			</div>

			{#if anyIncluded}
				<InfoMessage>
					{#snippet content()}
						Including changed branches will perform a workspace checkout. Your working directory
						files may change.
					{/snippet}
				</InfoMessage>
			{/if}

			{#if resolveError}
				<InfoMessage style="danger">
					{#snippet content()}
						{resolveError}
					{/snippet}
				</InfoMessage>
			{/if}
		</div>
	{/snippet}
	{#snippet controls(close)}
		<AsyncButton
			testId="workspace-divergence-modal-action-button"
			style="pop"
			loading={resolveMutation.current.isLoading}
			action={async () => await resolve(close)}
		>
			Apply
		</AsyncButton>
	{/snippet}
</Modal>

<style lang="postcss">
	.divergence-modal {
		display: flex;
		flex-direction: column;
		padding: 20px;
		gap: 16px;
	}

	.divergence-modal__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.divergence-modal__description {
		color: var(--text-2);
	}

	.divergence-modal__list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.divergence-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px;
		gap: 12px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
	}

	.divergence-item__info {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		gap: 2px;
	}

	.divergence-item__action {
		flex-shrink: 0;
	}
</style>

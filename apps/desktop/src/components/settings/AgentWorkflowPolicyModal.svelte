<script lang="ts">
	import { AGENTS_SERVICE } from "$lib/agents/agentsService";
	import { inject } from "@gitbutler/core/context";
	import { Button, Checkbox, Modal, Textbox, Tooltip } from "@gitbutler/ui";
	import type {
		PolicyOptions,
		SkillScope,
		WorkflowOptionId,
		WorkflowOptionInfo,
	} from "@gitbutler/but-sdk";

	type Props = {
		scope: SkillScope;
		projectId?: string;
	};

	const { scope, projectId }: Props = $props();

	const agentsService = inject(AGENTS_SERVICE);
	const policy = $derived(agentsService.policy({ scope, projectId }));
	const [savePolicy, saving] = agentsService.setPolicy;

	let modal = $state<Modal>();

	// Local edit buffer, seeded from the installed policy each time the modal
	// opens. Not derived: the user's in-progress edits must survive a
	// background refetch of the query behind it.
	let selected = $state<WorkflowOptionId[]>([]);
	let publishPhrase = $state("ship it");
	let branchPattern = $state("");
	let commitConvention = $state("");

	let open = $state(false);
	// Plain variable, not $state: the effect below both reads and writes it,
	// and it must not retrigger itself.
	let seeded = false;

	const policyState = $derived(policy.response);
	/** Options are described by the backend so the wording lives in one place. */
	const available = $derived(policyState?.available ?? []);

	/**
	 * Options collected under their group heading.
	 *
	 * Built by bucketing rather than by watching for the group to change
	 * between neighbours: the backend sends options in the order the CLI
	 * wizard presents them, which interleaves groups, so the naive approach
	 * repeats a heading every time the sequence returns to a group.
	 * Group order follows first appearance, which keeps it stable.
	 */
	const groups = $derived.by(() => {
		const order: WorkflowOptionInfo[][] = [];
		const seen = new Map<string, WorkflowOptionInfo[]>();
		for (const option of available) {
			let bucket = seen.get(option.group);
			if (!bucket) {
				bucket = [];
				seen.set(option.group, bucket);
				order.push(bucket);
			}
			bucket.push(option);
		}
		return order;
	});

	export function show() {
		seeded = false;
		open = true;
		modal?.show();
	}

	/**
	 * Seed the buffer once the policy has actually loaded.
	 *
	 * Doing this in `show()` is too early: the query is usually still in
	 * flight when the button is clicked, so the buffer would be filled from
	 * `undefined` and every box would render unchecked — including the ones
	 * that are on by default.
	 */
	$effect(() => {
		if (!open || seeded || !policyState) return;
		seeded = true;
		// `current` is null when no managed block exists yet, in which case the
		// defaults are what a fresh setup would have written.
		const current = policyState.current ?? policyState.defaults;
		selected = [...current.selected];
		publishPhrase = current.publishPhrase;
		branchPattern = current.branchPattern ?? "";
		commitConvention = current.commitConvention ?? "";
	});

	function has(id: WorkflowOptionId) {
		return selected.includes(id);
	}

	function toggle(id: WorkflowOptionId) {
		selected = has(id)
			? selected.filter((entry: WorkflowOptionId) => entry !== id)
			: [...selected, id];
	}

	/**
	 * A repo-local rule is rendered once and written everywhere the setup
	 * targets, so offering it globally would leak it into the user's global
	 * config.
	 */
	function disabled(repoLocalOnly: boolean) {
		return repoLocalOnly && scope !== "repository";
	}

	async function save(close: () => void) {
		const options: PolicyOptions = {
			selected: selected.filter((id: WorkflowOptionId) => {
				const option = available.find((entry: WorkflowOptionInfo) => entry.id === id);
				return !option || !disabled(option.repoLocalOnly);
			}),
			publishPhrase: publishPhrase.trim() || "ship it",
			// An empty box means "no preference", not an empty pattern.
			branchPattern: branchPattern.trim() || null,
			commitConvention: commitConvention.trim() || null,
		};
		await savePolicy({ scope, projectId, options });
		open = false;
		close();
	}
</script>

<Modal
	bind:this={modal}
	width="medium"
	title="Customize agent workflow"
	onSubmit={async (close) => await save(close)}
>
	<p class="text-12 text-body clr-text-2 m-b-12">
		These preferences are written into the GitButler section of your agent instruction files.
		{#if policyState?.diverged}
			<br />Your instruction files currently disagree; saving will make them consistent.
		{/if}
	</p>

	<div class="options">
		{#each groups as group, groupIndex (group[0]?.group)}
			<h4 class="group-heading text-13 text-semibold" class:group-heading--first={groupIndex === 0}>
				{group[0]?.groupTitle}
			</h4>
			{#each group as option (option.id)}
				{@const isDisabled = disabled(option.repoLocalOnly)}
				<div class="option" class:option--disabled={isDisabled}>
					<Checkbox
						id="workflow-{option.id}"
						checked={has(option.id)}
						disabled={isDisabled}
						onchange={() => toggle(option.id)}
					/>
					<label for="workflow-{option.id}" class="option__text">
						<span class="text-13 text-semibold">{option.label}</span>
						<span class="text-12 clr-text-2">
							{#if isDisabled && option.repoLocalHelp}
								<Tooltip text={option.repoLocalHelp}>
									<span>{option.repoLocalHelp}</span>
								</Tooltip>
							{:else}
								{option.help}
							{/if}
						</span>

						{#if has(option.id) && !isDisabled}
							{#if option.id === "publishPhrase"}
								<Textbox label="Phrase" bind:value={publishPhrase} placeholder="ship it" />
							{:else if option.id === "branchPattern"}
								<Textbox
									label="Pattern"
									bind:value={branchPattern}
									placeholder="<name>/<short-description>"
								/>
							{:else if option.id === "commitConvention"}
								<Textbox
									label="Convention"
									bind:value={commitConvention}
									placeholder="type(scope): summary"
								/>
							{/if}
						{/if}
					</label>
				</div>
			{/each}
		{/each}
	</div>

	{#snippet controls(close)}
		<Button
			kind="outline"
			type="reset"
			onclick={() => {
				open = false;
				close();
			}}>Cancel</Button
		>
		<Button style="pop" type="submit" loading={saving.current.isLoading}>Save</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	/* The list is taller than the viewport on smaller windows, and the modal
	   body does not scroll on its own, so the last option was unreachable. */
	.options {
		display: flex;
		flex-direction: column;
		max-height: 55vh;
		padding-right: 4px;
		overflow-y: auto;
		gap: 12px;
	}

	.group-heading {
		margin-top: 8px;
		padding-top: 12px;
		border-top: 1px solid var(--clr-border-3);
		color: var(--clr-text-2);
	}

	.group-heading--first {
		margin-top: 0;
		padding-top: 0;
		border-top: none;
	}

	.option {
		display: flex;
		align-items: flex-start;
		gap: 10px;
	}

	.option--disabled {
		opacity: 0.6;
	}

	.option__text {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
</style>

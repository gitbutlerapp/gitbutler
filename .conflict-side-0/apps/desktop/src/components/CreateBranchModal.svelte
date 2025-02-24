<script lang="ts">
	import BranchNameTextbox from '$components/BranchNameTextbox.svelte';
	import dependentBranchSvg from '$components/stackTabs/assets/dependent-branch.svg?raw';
	import newStackLefttSvg from '$components/stackTabs/assets/new-stack-left.svg?raw';
	import newStackRightSvg from '$components/stackTabs/assets/new-stack-right.svg?raw';
	import { autoSelectBranchCreationFeature } from '$lib/config/uiFeatureFlags';
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';

	import {
		Button,
		ElementId,
		Icon,
		Link,
		Modal,
		RadioButton,
		Select,
		SelectItem,
		TestId
	} from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		projectId: string;
		stackId?: string;
	};

	let { projectId, stackId }: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const [createNewStack, stackCreation] = stackService.newStack;
	const [createNewBranch, branchCreation] = stackService.newBranch;
	const { openGeneralSettings } = useSettingsModal();

	let createRefModal = $state<ReturnType<typeof Modal>>();
	let createRefName = $state<string>();
	let createRefType = $state<'stack' | 'dependent'>('stack');
	let selectedStackId = $state<string>();
	let branchNameInput = $state<ReturnType<typeof BranchNameTextbox>>();

	// Persisted preference for branch placement
	const addToLeftmost = persisted<boolean>(false, 'branch-placement-leftmost');

	let slugifiedRefName: string | undefined = $state();

	// Get all stacks in the workspace
	const allStacksQuery = $derived(stackService.stacks(projectId));
	const allStacks = $derived(allStacksQuery?.response ?? []);

	// Create options for the selector (stack represented by first branch name)
	const stackOptions = $derived(
		allStacks
			.map((stack) => {
				if (!stack.id) return;
				const firstBranchName = stack.heads[0]?.name ?? `Stack ${stack?.id.slice(0, 8)}`;
				return {
					label: firstBranchName,
					value: stack.id
				};
			})
			.filter(isDefined)
	);

	// Set default selected stack and handle if current selected stack is no longer available
	$effect(() => {
		if (stackOptions.length === 0) {
			selectedStackId = undefined;
			// If no stacks available and dependent is selected, switch to stack
			if (createRefType === 'dependent') {
				createRefType = 'stack';
			}
			return;
		}

		// If no stack selected or the currently selected stack doesn't exist, pick a default
		if (!selectedStackId || !stackOptions.some((option) => option.value === selectedStackId)) {
			// Default to current stack if it exists, otherwise first stack
			selectedStackId =
				stackId && allStacks.some((s) => s.id === stackId) ? stackId : stackOptions[0]?.value;
		}
	});

	function handleOptionSelect(event: Event) {
		const target = event.target as HTMLInputElement;
		createRefType = target.id === 'new-stack' ? 'stack' : 'dependent';
	}

	async function addNew() {
		if (createRefType === 'stack') {
			await createNewStack({
				projectId,
				branch: {
					name: slugifiedRefName,
					// If addToLeftmost is true, place at position 0 (leftmost)
					// Otherwise, leave undefined to append to the right
					order: $addToLeftmost ? 0 : undefined
				}
			});
			createRefModal?.close();
		} else {
			if (!selectedStackId || !slugifiedRefName) {
				// TODO: Add input validation.
				return;
			}
			await createNewBranch({
				projectId,
				stackId: selectedStackId,
				request: { targetPatch: undefined, name: slugifiedRefName }
			});
			createRefModal?.close();
		}

		// Reset the form if we're successful
		createRefName = undefined;
		selectedStackId = undefined;
	}

	const isAddingNew = $derived(stackCreation.current.isLoading || branchCreation.current.isLoading);

	export async function show(initialType?: 'stack' | 'dependent') {
		createRefModal?.show();
		createRefName = await stackService.fetchNewBranchName(projectId);

		// Select text after async value is loaded and DOM is updated
		if ($autoSelectBranchCreationFeature) {
			await branchNameInput?.selectAll();
		}
		// Reset selected stack to default
		selectedStackId = undefined;
		// Set branch type - default to 'stack' unless explicitly provided
		createRefType = initialType ?? 'stack';
	}

	export function close() {
		createRefModal?.close();
	}
</script>

<Modal bind:this={createRefModal} width={500} testId={TestId.CreateNewBranchModal}>
	<div class="content-wrap">
		<BranchNameTextbox
			bind:this={branchNameInput}
			label="New branch"
			id={ElementId.NewBranchNameInput}
			value={createRefName}
			autofocus
			onslugifiedvalue={(value) => (slugifiedRefName = value)}
		/>

		<div class="options-wrap" role="radiogroup" aria-label="Branch type selection">
			<!-- Option 1 -->
			<label for="new-stack" class="radio-label" class:radio-selected={createRefType === 'stack'}>
				<div class="radio-btn">
					<RadioButton
						checked={createRefType === 'stack'}
						name="create-new"
						id="new-stack"
						onchange={handleOptionSelect}
					/>
				</div>

				<div class="radio-content">
					<h3 class="text-14 text-bold text-body radio-title">Independent branch</h3>
					<p class="text-12 text-body radio-caption">
						Create an independent branch<br />in a new stack.
					</p>

					<div class="radio-illustration">
						{#if $addToLeftmost}
							{@html newStackLefttSvg}
						{:else}
							{@html newStackRightSvg}
						{/if}
					</div>
				</div>
			</label>
			<!-- Option 2 -->
			<label
				for="new-dependent"
				class="radio-label"
				class:radio-selected={createRefType === 'dependent'}
				class:disabled={allStacks.length === 0}
			>
				<div class="radio-btn">
					<RadioButton
						checked={createRefType === 'dependent'}
						name="create-new"
						id="new-dependent"
						disabled={allStacks.length === 0}
						onchange={handleOptionSelect}
					/>
				</div>

				<div class="radio-content">
					<h3 class="text-14 text-bold text-body radio-title">Dependent branch</h3>
					<p class="text-12 text-body radio-caption">
						{#if allStacks.length === 0}
							Create a branch that depends<br />on another stack (none available).
						{:else}
							Create a branch that depends<br />on a selected stack.
						{/if}
					</p>

					<div class="radio-illustration">
						{@html dependentBranchSvg}
					</div>
				</div>
			</label>
		</div>

		{#if createRefType === 'dependent'}
			<Select
				options={stackOptions}
				value={selectedStackId}
				label="Add to stack"
				disabled={stackOptions.length <= 1}
				placeholder="Select a stack..."
				onselect={(value) => (selectedStackId = value)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === selectedStackId} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}

		<div class="text-12 text-body clr-text-2 radio-aditional-info">
			<span>└</span>

			<p>
				{#if createRefType === 'stack'}
					The new branch will be applied in parallel with other stacks in the workspace.
					<br />
					Adjust branch placement and preferences in
					<button
						type="button"
						class="settings-link underline-dotted"
						onclick={() => {
							createRefModal?.close();
							openGeneralSettings('lanes-and-branches');
						}}
					>
						Settings → Lanes & branches
					</button>
				{:else}
					Creates a branch that depends on a selected stack.
					<br />
					A stack's top branches also have a
					<i class="create-dependent-icon"><Icon name="new-dep-branch" /></i> icon to create dependent
					branches.
				{/if}
			</p>
		</div>
	</div>

	{#snippet controls(close)}
		<div class="footer">
			<span class="text-12 text-body footer-text"
				>See more: <Link
					href="https://docs.gitbutler.com/features/branch-management/stacked-branches"
					>Stacked vs. Dependent</Link
				></span
			>

			<div class="footer__controls">
				<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
				<Button
					style="pop"
					type="submit"
					onclick={addNew}
					disabled={!createRefName || (createRefType === 'dependent' && !selectedStackId)}
					loading={isAddingNew}
					testId={TestId.ConfirmSubmit}
				>
					Create branch
				</Button>
			</div>
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	/* MODAL WINDOW */
	.content-wrap {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.options-wrap {
		display: flex;
		gap: 8px;
	}

	.settings-link {
		padding: 0;
		border: none;
		background: none;
		color: var(--clr-link);
		font: inherit;
		cursor: pointer;

		&:hover {
			color: var(--clr-link-hover);
		}
	}

	.radio-label {
		/* variables */
		--btn-bg: var(--clr-btn-ntrl-outline-bg);
		--btn-bg-opacity: 0;
		--btn-border-clr: var(--clr-btn-ntrl-outline);
		--btn-border-opacity: var(--opacity-btn-outline);
		--content-opacity: 1;
		/* illustration */
		--image-outline: var(--clr-border-2);
		--image-text: var(--clr-text-3);
		--image-accent-outline: var(--clr-text-3);
		--image-accent-bg: var(--clr-bg-2);
		/*  */
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		padding: 14px 14px 0;
		gap: 4px;
		border: 1px solid
			color-mix(
				in srgb,
				var(--btn-border-clr, transparent),
				transparent calc((1 - var(--btn-border-opacity, 1)) * 100%)
			);

		border-radius: var(--radius-m);
		background: color-mix(
			in srgb,
			var(--btn-bg, transparent),
			transparent calc((1 - var(--btn-bg-opacity, 1)) * 100%)
		);
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);

		&:not(.radio-selected)&:not(.disabled):hover {
			--btn-bg-opacity: 0.14;
		}

		&.disabled {
			--btn-bg: var(--clr-btn-ntrl-outline-bg);
			--btn-bg-opacity: 0.1;
			--btn-border-clr: var(--clr-btn-ntrl-outline);
			--btn-border-opacity: 0.1;
			--image-outline: var(--clr-border-1);
			--image-accent-outline: var(--clr-text-3);
			--image-accent-bg: var(--clr-bg-2);
			--content-opacity: 0.5;
			cursor: not-allowed;
		}
	}

	.radio-content {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		height: 100%;
		gap: 4px;
		opacity: var(--content-opacity);
	}

	.radio-btn {
		display: flex;
		position: absolute;
		top: 12px;
		right: 12px;
	}

	.radio-caption {
		opacity: 0.7;
	}

	.radio-illustration {
		display: flex;
		align-items: flex-end;
		height: 100%;
		margin-top: 16px;
	}

	.radio-aditional-info {
		display: flex;
		gap: 8px;
	}

	.create-dependent-icon {
		display: inline-flex;
		align-items: center;
		margin: 0 2px;
		transform: translateY(4px);
	}

	/* MODIFIERS */
	.radio-selected {
		--btn-bg: var(--clr-theme-pop-bg);
		--btn-bg-opacity: 1;
		--btn-border-clr: var(--clr-btn-pop-outline);
		--btn-border-opacity: 0.6;
		/* illustration */
		--image-outline: var(--clr-border-1);
		--image-accent-outline: var(--clr-theme-pop-element);
		--image-accent-bg: var(--clr-theme-pop-bg);
	}

	/* FOOTER */
	.footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		gap: 16px;
		color: var(--clr-text-2);
	}

	.footer__controls {
		display: flex;
		gap: 8px;
	}
</style>

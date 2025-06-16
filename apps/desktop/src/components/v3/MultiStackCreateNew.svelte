<script lang="ts">
	import dependentBranchSvg from '$components/v3/stackTabs/assets/dependent-branch.svg?raw';
	import newStackSvg from '$components/v3/stackTabs/assets/new-stack.svg?raw';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { sleep } from '$lib/utils/sleep';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import RadioButton from '@gitbutler/ui/RadioButton.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { slugify } from '@gitbutler/ui/utils/string';

	type Props = {
		el?: HTMLButtonElement;
		scrollerEl?: HTMLDivElement;
		projectId: string;
		stackId?: string;
		noStacks: boolean;
	};

	let { el = $bindable(), scrollerEl, projectId, stackId, noStacks }: Props = $props();
	const [stackService, uiState, uncommittedService] = inject(
		StackService,
		UiState,
		UncommittedService
	);
	const projectState = $derived(uiState.project(projectId));
	const [createNewStack, stackCreation] = stackService.newStack;
	const [createNewBranch, branchCreation] = stackService.newBranch;

	const treeChanges = $derived(uncommittedService.changesByStackId(null));
	const changesToCommit = $derived(treeChanges.current.length > 0);

	let createRefModal = $state<ReturnType<typeof Modal>>();
	let createRefName = $state<string>();
	let createRefType = $state<'stack' | 'dependent'>('stack');

	const slugifiedRefName = $derived(createRefName && slugify(createRefName));
	const generatedNameDiverges = $derived(!!createRefName && slugifiedRefName !== createRefName);

	const firstBranchResult = $derived(
		stackId ? stackService.branchAt(projectId, stackId, 0) : undefined
	);
	const firstBranchName = $derived(firstBranchResult?.current?.data?.name);

	const exclusiveAction = $derived(projectState.exclusiveAction.current);

	function handleOptionSelect(event: Event) {
		const target = event.target as HTMLInputElement;
		createRefType = target.id === 'new-stack' ? 'stack' : 'dependent';
	}

	async function addNew() {
		if (createRefType === 'stack') {
			const stack = await createNewStack({
				projectId,
				branch: { name: slugifiedRefName }
			});
			// Why is there a timing thing going on here? Withou sleep you end
			// up on stacks[0] after creating a new one.
			await sleep(50);
			uiState.project(projectId).stackId.set(stack.id);
			createRefModal?.close();
		} else {
			if (!stackId || !slugifiedRefName) {
				// TODO: Add input validation.
				return;
			}
			await createNewBranch({
				projectId,
				stackId,
				request: { targetPatch: undefined, name: slugifiedRefName }
			});
			createRefModal?.close();
		}

		// Reset the branch name if we're successful
		createRefName = undefined;
	}

	const isAddingNew = $derived(stackCreation.current.isLoading || branchCreation.current.isLoading);

	function handleArrowNavigation(event: KeyboardEvent) {
		if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
			event.preventDefault();
			const target = scrollerEl as HTMLDivElement;
			// first child
			const firstChild = target.firstElementChild as HTMLButtonElement;
			// last child
			const lastChild = target.lastElementChild as HTMLButtonElement;

			if (event.key === 'ArrowRight') {
				firstChild.focus();
			} else if (event.key === 'ArrowLeft') {
				lastChild.focus();
			}
		}
	}

	async function showAndPrefillName() {
		createRefModal?.show();
		createRefName = (await stackService.newBranchName(projectId))?.data ?? '';
	}

	// TODO: it would be nice to remember the last selected option for the next time the modal is opened
</script>

<div class="multi-stack-create-new">
	{#if (exclusiveAction?.type !== 'commit' && exclusiveAction?.stackId) || changesToCommit}
		<div class="multi-stack-create-new__button-wrap">
			<Button
				type="button"
				onclick={() => {
					projectState.exclusiveAction.set({ type: 'commit' });
					uncommittedService.checkAll(null);
				}}
				icon="commit"
				onkeydown={handleArrowNavigation}
				testId={TestId.CommitToNewBranchButton}
				kind="outline"
			>
				Commit to new branch
			</Button>
		</div>
	{/if}

	<div class="multi-stack-create-new__button-wrap">
		<Button
			type="button"
			onclick={() => showAndPrefillName()}
			onkeydown={handleArrowNavigation}
			testId={TestId.CreateStackButton}
			kind="outline"
			icon="plus-small"
		>
			New branch
		</Button>
	</div>
</div>

<Modal bind:this={createRefModal} width={500}>
	<div class="content-wrap">
		<Textbox
			label="New branch"
			id="newRemoteName"
			bind:value={createRefName}
			autofocus
			helperText={generatedNameDiverges ? `Will be created as '${slugifiedRefName}'` : undefined}
		/>

		<div class="options-wrap">
			<!-- Option 1 -->
			<label for="new-stack" class="radio-label" class:radio-selected={createRefType === 'stack'}>
				<div class="radio-btn">
					<RadioButton checked name="create-new" id="new-stack" onchange={handleOptionSelect} />
				</div>

				<div class="radio-content">
					<h3 class="text-13 text-bold text-body radio-title">Independent branch</h3>
					<p class="text-12 text-body radio-caption">
						Create an independent branch<br />in a new stack.
					</p>

					<div class="radio-illustration">
						{@html newStackSvg}
					</div>
				</div>
			</label>
			<!-- Option 2 -->
			<label
				for="new-dependent"
				class="radio-label"
				class:disabled={noStacks}
				class:radio-selected={createRefType === 'dependent'}
			>
				<div class="radio-btn">
					<RadioButton
						disabled={noStacks}
						name="create-new"
						id="new-dependent"
						onchange={handleOptionSelect}
					/>
				</div>

				<div class="radio-content">
					<h3 class="text-13 text-bold text-body radio-title">Dependent branch</h3>
					<p class="text-12 text-body radio-caption">
						Create a branch that depends on<br />the branches in the current stack.
					</p>

					<div class="radio-illustration">
						{@html dependentBranchSvg}
					</div>
				</div>
			</label>
		</div>

		<span class="text-12 text-body radio-aditional-info"
			>{createRefType === 'stack'
				? '└ The new branch will be applied in parallel with other stacks in the workspace.'
				: `└ The new branch will be added on top of \`${firstBranchName}\``}</span
		>
	</div>

	{#snippet controls(close)}
		<div class="footer">
			<span class="text-12 text-body footer-text"
				>See more: <Link
					target="_blank"
					rel="noreferrer"
					href="https://docs.gitbutler.com/features/stacked-branches#comparison-to-virtual-branches"
					>Stacked vs. Dependent</Link
				></span
			>

			<div class="footer__controls">
				<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
				<Button
					style="pop"
					type="submit"
					onclick={addNew}
					disabled={!createRefName}
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
	.multi-stack-create-new {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
	}

	.multi-stack-create-new__button-wrap {
		display: flex;
		border-radius: var(--radius-btn);
		background-color: var(--clr-bg-2);
	}

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

	.radio-label {
		--btn-bg: var(--clr-btn-ntrl-outline-bg);
		--btn-bg-opacity: 0;
		--btn-border-clr: var(--clr-btn-ntrl-outline);
		--btn-border-opacity: var(--opacity-btn-outline);
		--content-opacity: 1;
		/* illustration */
		--illustration-outline: var(--clr-border-2);
		--illustration-text: var(--clr-text-3);
		--illustration-accent-outline: var(--clr-text-3);
		--illustration-accent-bg: var(--clr-bg-2);
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
			--illustration-outline: var(--clr-text-3);
			--illustration-text: var(--clr-text-3);
			--illustration-accent-outline: var(--clr-text-3);
			--illustration-accent-bg: var(--clr-bg-2);
			--content-opacity: 0.5;
			cursor: not-allowed;
		}
	}

	.radio-content {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		height: 100%;
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
		margin-top: 20px;
	}

	.radio-aditional-info {
		color: var(--clr-text-2);
	}

	/* MODIFIERS */
	.radio-selected {
		--btn-bg: var(--clr-theme-pop-bg);
		--btn-bg-opacity: 1;
		--btn-border-clr: var(--clr-btn-pop-outline);
		/* illustration */
		--illustration-outline: var(--clr-text-3);
		--illustration-text: var(--clr-text-2);
		--illustration-accent-outline: var(--clr-theme-pop-element);
		--illustration-accent-bg: var(--clr-theme-pop-bg);
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
</style>

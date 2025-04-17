<script lang="ts">
	import dependentBranchSvg from '$components/v3/stackTabs/assets/dependent-branch.svg?raw';
	import newStackSvg from '$components/v3/stackTabs/assets/new-stack.svg?raw';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import RadioButton from '@gitbutler/ui/RadioButton.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { slugify } from '@gitbutler/ui/utils/string';
	import { goto } from '$app/navigation';

	type Props = {
		el?: HTMLButtonElement;
		scrollerEl?: HTMLDivElement;
		projectId: string;
		stackId?: string;
		noStacks: boolean;
	};

	let { el = $bindable(), scrollerEl, projectId, stackId, noStacks }: Props = $props();
	const stackService = getContext(StackService);
	const [createNewStack, stackCreation] = stackService.newStack;
	const [createNewBranch, branchCreation] = stackService.newBranch;

	let createRefModal = $state<ReturnType<typeof Modal>>();
	let createRefName = $state<string>();
	let createRefType = $state<'stack' | 'dependent'>('stack');

	const slugifiedRefName = $derived(createRefName && slugify(createRefName));
	const generatedNameDiverges = $derived(!!createRefName && slugifiedRefName !== createRefName);

	const firstBranchResult = $derived(
		stackId ? stackService.branchAt(projectId, stackId, 0) : undefined
	);
	const firstBranchName = $derived(firstBranchResult?.current?.data?.name);

	function handleOptionSelect(event: Event) {
		const target = event.target as HTMLInputElement;
		createRefType = target.id === 'new-stack' ? 'stack' : 'dependent';
	}

	async function addNew() {
		if (createRefType === 'stack') {
			const stack = await createNewStack({
				projectId,
				branch: { name: createRefName }
			});
			goto(stackPath(projectId, stack.id));
			createRefModal?.close();
		} else {
			if (!stackId || !createRefName) {
				// TODO: Add input validation.
				return;
			}
			await createNewBranch({
				projectId,
				stackId,
				request: { targetPatch: undefined, name: createRefName }
			});
			goto(stackPath(projectId, stackId));
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

	// TODO: it would be nice to remember the last selected option for the next time the modal is opened
</script>

<button
	aria-label="new stack"
	type="button"
	class="new-stack-btn"
	class:no-stacks={noStacks}
	onclick={() => createRefModal?.show()}
	bind:this={el}
	onkeydown={handleArrowNavigation}
>
	{#if noStacks}
		<p class="text-13">Create new branch</p>
	{/if}

	<svg width="16" height="16" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
		<path
			d="M0 10H20M10 0L10 20"
			stroke="currentColor"
			opacity="var(--plus-icon-opacity)"
			stroke-width="1.5"
			vector-effect="non-scaling-stroke"
		/>
	</svg>
</button>

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
				>
					{#if createRefType === 'stack'}
						Add new stack
					{:else}
						Add dependent branch
					{/if}
				</Button>
			</div>
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.new-stack-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: 0 var(--radius-ml) 0 0;
		height: 100%;
		padding: 12px 15px;
		background: var(--clr-stack-tab-inactive);
		color: var(--clr-text-2);
		--plus-icon-opacity: 0.8;
		transition:
			color var(--transition-fast),
			background var(--transition-fast);
		gap: 10px;

		&:hover,
		&:focus {
			--plus-icon-opacity: 1;
		}

		&:hover {
			background: var(--clr-stack-tab-inactive-hover);
		}

		&:focus {
			outline: none;
			background: var(--clr-stack-tab-active);
		}

		&.no-stacks {
			border-top-left-radius: var(--radius-ml);
		}
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

		position: relative;
		display: flex;
		flex: 1;
		flex-direction: column;
		padding: 14px 14px 0;
		gap: 4px;
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);

		border-radius: var(--radius-m);
		background: color-mix(
			in srgb,
			var(--btn-bg, transparent),
			transparent calc((1 - var(--btn-bg-opacity, 1)) * 100%)
		);
		border: 1px solid
			color-mix(
				in srgb,
				var(--btn-border-clr, transparent),
				transparent calc((1 - var(--btn-border-opacity, 1)) * 100%)
			);

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
		position: absolute;
		right: 12px;
		top: 12px;
		display: flex;
	}

	.radio-caption {
		opacity: 0.7;
	}

	.radio-illustration {
		display: flex;
		align-items: flex-end;
		margin-top: 20px;
		height: 100%;
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
		justify-content: space-between;
		align-items: center;
		width: 100%;
		gap: 16px;
		color: var(--clr-text-2);
	}
</style>

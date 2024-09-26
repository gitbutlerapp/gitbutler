<script lang="ts" module>
	export interface CreatePrParams {
		title: string;
		body: string;
		draft: boolean;
	}
</script>

<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import Markdown from '$lib/components/Markdown.svelte';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { autoHeight } from '$lib/utils/autoHeight';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { DetailedPullRequest } from '$lib/gitHost/interface/types';

	interface BaseProps {
		type: 'display' | 'preview';
	}

	interface DisplayProps extends BaseProps {
		type: 'display';
		pr: DetailedPullRequest;
	}

	interface PreviewProps extends BaseProps {
		type: 'preview';
		draft: boolean;
		onCreatePr: (p: CreatePrParams) => void;
	}

	type Props = DisplayProps | PreviewProps;

	let props: Props = $props();

	const project = getContext(Project);
	const branchStore = getContextStore(VirtualBranch);
	const prService = getGitHostPrService();

	let modal = $state<Modal>();
	let bodyTextArea = $state<HTMLTextAreaElement | null>(null);
	let isEditing = $state<boolean>(false);
	const branch = $derived($branchStore);
	const prTemplatePath = $derived(project.git_host.pullRequestTemplatePath);
	let pullRequestTemplateBody = $state<string | undefined>(undefined);

	const defaultTitle: string = $derived.by(() => {
		if (props.type === 'display') return props.pr.title;
		// In case of a single commit, use the commit summary for the title
		if (branch.commits.length === 1) {
			const commit = branch.commits[0];
			return commit?.descriptionTitle ?? '';
		} else {
			return branch.name;
		}
	});

	const defaultBody: string = $derived.by(() => {
		if (props.type === 'display') return props.pr.body ?? '';
		if (pullRequestTemplateBody) return pullRequestTemplateBody;
		// In case of a single commit, use the commit description for the body
		if (branch.commits.length === 1) {
			const commit = branch.commits[0];
			return commit?.descriptionBody ?? '';
		} else {
			return '';
		}
	});

	let inputBody = $state<string | undefined>(undefined);
	let inputTitle = $state<string | undefined>(undefined);
	const actualBody = $derived<string>(inputBody ?? defaultBody);
	const actualTitle = $derived<string>(inputTitle ?? defaultTitle);

	// Fetch PR template content
	$effect(() => {
		if ($prService && pullRequestTemplateBody === undefined) {
			$prService.pullRequestTemplateContent(prTemplatePath, project.id).then((template) => {
				pullRequestTemplateBody = template;
			});
		}
	});

	function updateFieldsHeight() {
		if (bodyTextArea) autoHeight(bodyTextArea);
	}

	function handleCreatePR(close: () => void) {
		if (props.type !== 'preview') return;
		props.onCreatePr({
			title: actualTitle,
			body: actualBody,
			draft: props.draft
		});
		close();
	}

	function toggleEdit() {
		isEditing = !isEditing;
	}

	function onClose() {
		isEditing = false;
		inputBody = undefined;
	}

	export function show() {
		modal?.show();
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		}
	};
</script>

<Modal bind:this={modal} width="large" noPadding {onClose}>
	{#snippet children(_, close)}
		<ScrollableContainer maxHeight="70vh">
			<div class="pr-modal__content">
				<div class="card">
					<div
						class="card__header text-14 text-body text-semibold pr-modal__header"
						class:editing={isEditing}
					>
						{#if isEditing}
							<div class="text-input pr-modal__title-input-wrapper">
								<input
									type="text"
									class="text-13 text-body text-semibold pr-modal__title-input"
									value={actualTitle}
									oninput={(e) => {
										inputTitle = e.currentTarget.value;
									}}
								/>
							</div>
						{:else if actualTitle}
							{actualTitle}
						{:else}
							<span class="text-clr2"> No title provided.</span>
						{/if}
					</div>
					{#if isEditing}
						<div
							class="pr-modal__body-input-wrapper text-input"
							use:resizeObserver={updateFieldsHeight}
						>
							<textarea
								bind:this={bodyTextArea}
								value={actualBody}
								onfocus={(e) => autoHeight(e.currentTarget)}
								oninput={(e) => {
									inputBody = e.currentTarget.value;
									autoHeight(e.currentTarget);
								}}
								class="text-13 text-body text-semibold pr-modal__body-input"
							></textarea>
						</div>
					{:else if actualBody}
						<div class="card__content text-13 text-body">
							<Markdown content={actualBody} />
						</div>
					{:else}
						<div class="card__content text-13 text-body text-clr2">No PR description.</div>
					{/if}
				</div>
			</div>
		</ScrollableContainer>
		<div class="pr-modal__footer">
			{#if props.type === 'preview'}
				<Button style="ghost" outline onclick={close}>Cancel</Button>
				<Button style="neutral" kind="solid" onclick={toggleEdit}
					>{isEditing ? 'Done' : 'Edit'}</Button
				>
				<Button style="pop" kind="solid" disabled={isEditing} onclick={() => handleCreatePR(close)}
					>{props.draft ? 'Create Draft PR' : 'Create PR'}</Button
				>
			{:else if props.type === 'display'}
				<Button style="ghost" outline onclick={close}>Done</Button>
			{/if}
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.pr-modal__content {
		padding: 16px;
	}

	.pr-modal__header {
		position: sticky;
		top: 0;
		background: var(--clr-bg-1);
		border-top-left-radius: var(--radius-m);
		border-top-right-radius: var(--radius-m);
		&.editing {
			padding: 8px;
		}
	}

	.pr-modal__title-input-wrapper {
		display: flex;
		position: relative;
		width: 100%;
		flex-direction: column;
		gap: 4px;
	}
	.pr-modal__title-input {
		width: 100%;
		border: none;
		background: none;
		outline: none;
	}

	.pr-modal__body-input-wrapper {
		display: flex;
		position: relative;
		padding: 16px;
		margin: 8px;
		flex-direction: column;
		gap: 4px;
	}

	.pr-modal__body-input {
		overflow: hidden;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 16px;
		background: none;
		resize: none;

		&:focus {
			outline: none;
		}

		&::placeholder {
			color: oklch(from var(--clr-scale-ntrl-30) l c h / 0.4);
		}
	}

	.pr-modal__footer {
		display: flex;
		width: 100%;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		border-bottom-left-radius: var(--radius-l);
		border-bottom-right-radius: var(--radius-l);
	}
</style>

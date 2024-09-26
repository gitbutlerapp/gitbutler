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
	import { getContext, getContextStore } from '$lib/utils/context';
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
	const branch = $derived($branchStore);
	const prTemplatePath = $derived(project.git_host.pullRequestTemplatePath);
	let pullRequestTemplateBody = $state<string | undefined>(undefined);

	const previewTitle: string | undefined = $derived.by(() => {
		if (props.type !== 'preview') return undefined;
		// In case of a single commit, use the commit summary for the title
		if (branch.commits.length === 1) {
			const commit = branch.commits[0];
			return commit?.descriptionTitle ?? '';
		} else {
			return branch.name;
		}
	});

	const previewBody: string | undefined = $derived.by(() => {
		if (props.type !== 'preview') return undefined;
		if (pullRequestTemplateBody) return pullRequestTemplateBody;
		// In case of a single commit, use the commit description for the body
		if (branch.commits.length === 1) {
			const commit = branch.commits[0];
			return commit?.descriptionBody ?? '';
		} else {
			return '';
		}
	});

	$effect(() => {
		if ($prService && pullRequestTemplateBody === undefined) {
			$prService.pullRequestTemplateContent(prTemplatePath, project.id).then((template) => {
				pullRequestTemplateBody = template;
			});
		}
	});

	function handleCreatePR(close: () => void) {
		if (props.type !== 'preview') return;
		props.onCreatePr({ title: previewTitle ?? '', body: previewBody ?? '', draft: props.draft });
		close();
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

<!-- PREVIEW MODAL -->
{#if props.type === 'preview'}
	<Modal bind:this={modal} width="large" noPadding onSubmit={handleCreatePR}>
		{#snippet children(_, close)}
			<ScrollableContainer maxHeight="70vh">
				<div class="pr-modal__content">
					<div class="card">
						<div class="card__header text-14 text-body text-semibold pr-modal__header">
							{#if previewTitle}
								{previewTitle}
							{:else}
								<span class="text-clr2"> No title provided.</span>
							{/if}
						</div>
						{#if previewBody}
							<div class="card__content text-13 text-body">
								<Markdown content={previewBody} />
							</div>
						{:else}
							<div class="card__content text-13 text-body text-clr2">No PR description.</div>
						{/if}
					</div>
				</div>
			</ScrollableContainer>
			<div class="pr-modal__footer">
				<Button style="ghost" outline onclick={close}>Cancel</Button>
				<Button style="pop" type="submit" kind="solid"
					>{props.draft ? 'Create Draft PR' : 'Create PR'}</Button
				>
			</div>
		{/snippet}
	</Modal>
{/if}

<!-- DISPLAY -->
{#if props.type === 'display'}
	<Modal bind:this={modal} width="large" noPadding>
		{#snippet children(_, close)}
			<ScrollableContainer maxHeight="70vh">
				<div class="pr-modal__content">
					<div class="card">
						<div class="card__header text-14 text-body text-semibold pr-modal__header">
							{props.pr.title}
						</div>
						{#if props.pr.body}
							<div class="card__content text-13 text-body">
								<Markdown content={props.pr.body} />
							</div>
						{:else}
							<div class="card__content text-13 text-body text-clr2">No PR description.</div>
						{/if}
					</div>
				</div>
			</ScrollableContainer>
			<div class="pr-modal__footer">
				<Button style="ghost" outline onclick={close}>Done</Button>
			</div>
		{/snippet}
	</Modal>
{/if}

<style>
	.pr-modal__content {
		padding: 16px;
	}

	.pr-modal__header {
		position: sticky;
		top: 0;
		background: var(--clr-bg-1);
		border-top-left-radius: var(--radius-m);
		border-top-right-radius: var(--radius-m);
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

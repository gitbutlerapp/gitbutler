<script lang="ts">
	import Markdown from '$lib/components/Markdown.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { DetailedPullRequest } from '$lib/gitHost/interface/types';

	interface Props {
		pr: DetailedPullRequest;
	}

	let { pr }: Props = $props();

	let modal = $state<Modal>();

	export function show() {
		modal?.show();
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		}
	};
</script>

<Modal bind:this={modal} width="large" noPadding>
	{#snippet children(_, close)}
		<ScrollableContainer maxHeight="70vh">
			<div class="pr-modal__content">
				<div class="card">
					<div class="card__header text-14 text-body text-semibold pr-modal__header">
						{pr.title}
					</div>
					{#if pr.body}
						<div class="card__content text-13 text-body">
							<Markdown content={pr.body} />
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

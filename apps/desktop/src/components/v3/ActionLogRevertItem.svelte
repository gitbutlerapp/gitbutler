<script lang="ts">
	import DataContextMenu from '$components/v3/DataContextMenu.svelte';
	import ActionService from '$lib/actions/actionService.svelte';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import type { ButlerAction } from '$lib/actions/types';

	type Props = {
		projectId: string;
		action: ButlerAction & { action: { type: 'revertAction' } };
		last: boolean;
		loadNextPage: () => void;
	};

	const { action, last, projectId, loadNextPage }: Props = $props();

	// An ActionLogItem (for now) is representing both the git changes that
	// happened but also the file changes that happened between this action and
	// the previous one.
	//
	// Diffing `previous.snapshotAfter` and `action.snapshotBefore` gives us the
	// changes that happend on disk between these two events.

	const user = getContextStore(User);
	const actionService = getContext(ActionService);
	const [revertSnapshot] = actionService.revertSnapshot;

	async function restore(id: string, description: string) {
		await revertSnapshot({ projectId, snapshot: id, description });
		// In some cases, restoring the snapshot doesnt update the UI correctly
		// Until we have that figured out, we need to reload the page.
		location.reload();
	}

	let lastIntersector = $state<HTMLElement>();

	$effect(() => {
		if (!lastIntersector) return;
		const observer = new IntersectionObserver((data) => {
			if (data.at(0)?.isIntersecting) {
				loadNextPage();
			}
		});
		observer.observe(lastIntersector);
		return () => observer.disconnect();
	});
	let showActions = $state(false);
	let showActionsTarget = $state<HTMLElement>();
</script>

<DataContextMenu
	bind:open={showActions}
	items={[
		[
			{
				label: 'Undo revert',
				onclick: async () => await restore(action.action.subject.snapshot, 'Undid previous revert')
			}
		]
	]}
	target={showActionsTarget}
/>

<div class="action-item">
	<div class="action-item__robot">
		{#if $user?.picture}
			<img class="user-icon__image" src={$user.picture} alt="" referrerpolicy="no-referrer" />
		{:else}
			<Icon name="profile" />
		{/if}
	</div>
	<div class="action-item__content">
		<div class="action-item__content__header">
			<div>
				<p class="text-13 text-bold">Revert action</p>
				<span class="text-13 text-greyer"
					><TimeAgo date={new Date(action.createdAt)} addSuffix /></span
				>
			</div>
			<div bind:this={showActionsTarget}>
				<Button icon="kebab" size="tag" kind="outline" onclick={() => (showActions = true)} />
			</div>
		</div>
		<span class="text-14 text-darkgrey">
			<Markdown content={action.action.subject.description} />
		</span>
		{#if last}
			<div bind:this={lastIntersector}></div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.action-item__robot {
		width: 30px;
		min-width: 30px;
		height: 30px;
		padding: 2px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);

		> img {
			border-radius: var(--radius-s);
		}
	}

	.action-item {
		display: flex;

		align-items: flex-start;

		gap: 14px;
	}

	.action-item__content__header {
		display: flex;
		align-items: flex-start;

		> div:first-of-type {
			flex-grow: 1;
		}

		> div {
			display: flex;
			flex-wrap: wrap;

			align-items: center;
			gap: 8px;
		}
	}

	.action-item__content {
		display: flex;

		flex-grow: 1;
		flex-direction: column;
		gap: 8px;
	}

	.text-darkgrey {
		color: var(--clr-core-ntrl-20);
		text-decoration-color: var(--clr-core-ntrl-20);
	}

	.text-greyer {
		color: var(--clr-text-3);
	}
</style>

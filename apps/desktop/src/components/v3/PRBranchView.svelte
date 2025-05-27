<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';

	type Props = {
		projectId: string;
		prNumber: number;
		onerror?: (error: unknown) => void;
	};
	const { projectId, prNumber, onerror }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);
	const prResult = $derived(prService?.get(prNumber, { forceRefetch: true }));
	const unitSymbol = $derived(prService?.unit.symbol ?? '');
</script>

<ReduxResult result={prResult?.current} {projectId} {onerror}>
	{#snippet children(pr, { projectId })}
		<Drawer {projectId}>
			<div class="pr">
				<div class="pr-header">
					<h2 class="text-14 text-semibold pr-title">
						{pr.title}
						<span class="pr-link-container">
							<Link target="_blank" rel="noopener noreferrer" href={pr.htmlUrl}>
								{unitSymbol}{pr.number}
							</Link>
						</span>
					</h2>
					{#if pr.draft}
						<Badge size="tag" style="neutral" icon="draft-pr-small">Draft</Badge>
					{:else}
						<Badge size="tag" style="success" icon="pr-small">Open</Badge>
					{/if}
				</div>

				<div class="text-13">
					<span class="text-bold">
						{pr.author?.name}
					</span>
					wants to merge into
					<span class="code-string">
						{pr.baseBranch}
					</span>
					from
					<span class="code-string">
						{pr.sourceBranch}
					</span>
				</div>
				{#if pr.body}
					<Markdown content={pr.body} />
				{/if}
			</div>
		</Drawer>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.pr {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.pr-header {
		display: flex;
		flex-direction: column;
		align-items: start;
		gap: 8px;
	}
</style>

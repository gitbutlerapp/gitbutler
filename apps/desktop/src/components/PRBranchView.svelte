<script lang="ts">
	import Drawer from '$components/Drawer.svelte';
	import PrStatusBadge from '$components/PrStatusBadge.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Avatar, Link, Markdown, TestId } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		prNumber: number;
		onerror?: (error: unknown) => void;
	};
	const { projectId, prNumber, onerror }: Props = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const prService = $derived(forge.current.prService);
	const prQuery = $derived(prService?.get(prNumber, { forceRefetch: true }));
</script>

<ReduxResult result={prQuery?.result} {projectId} {onerror}>
	{#snippet children(pr)}
		<Drawer testId={TestId.PRBranchDrawer} rounded>
			{#snippet header()}
				<h3 class="text-14 text-semibold truncate">
					<span class="clr-text-2">{forge.reviewUnitAbbr} {forge.reviewUnitSymbol}{pr.number}:</span
					>
					<span> {pr.title}</span>
				</h3>
			{/snippet}

			<div class="pr-content">
				<div class="pr-request-data">
					<Avatar
						size="medium"
						srcUrl={pr.author?.gravatarUrl || ''}
						username={pr.author?.name || 'Unknown Author'}
					/>
					<div class="pr-request-data__wrapper">
						<p class="pr-request-data__sentence text-13">
							<span class="text-bold clr-text-1">
								{pr.author?.name}
							</span>
							wants to merge into
							<span class="code-string text-semibold">
								{pr.baseBranch}
							</span>
							from
							<span class="code-string text-semibold">
								{pr.sourceBranch}
							</span>
						</p>

						<div class="pr-request-data__details text-12">
							<PrStatusBadge {pr} />
							<span class="pr-request-data__divider">•</span>
							<span>No remote</span>

							<span class="pr-request-data__divider">•</span>
							<Link href={pr.htmlUrl}>Open in browser</Link>
						</div>
					</div>
				</div>

				{#if pr.body}
					<div class="pr-body text-13 text-body">
						<Markdown content={pr.body} />
					</div>
				{/if}
			</div>
		</Drawer>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.pr-content {
		display: flex;
		flex-direction: column;
	}

	.pr-request-data {
		display: flex;
		flex-direction: row;
		width: 100%;
		padding: 16px;
		gap: 10px;
	}

	.pr-request-data__wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 10px;
	}

	.pr-request-data__sentence {
		color: var(--clr-text-2);
		line-height: 140%;
	}

	.pr-request-data__details {
		display: flex;
		flex-direction: row;
		align-items: center;
		gap: 8px;
		color: var(--clr-text-2);
	}

	.pr-request-data__divider {
		color: var(--clr-text-3);
	}

	.code-string {
		color: var(--clr-text-1);
	}

	.pr-body {
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>

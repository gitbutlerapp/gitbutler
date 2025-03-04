<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import LinkButton from '@gitbutler/ui/LinkButton.svelte';
	import type { Readable } from 'svelte/store';

	interface Props {
		mergedIncorrectly?: Readable<boolean>;
		isPushed: boolean;
		hasParent: boolean;
		parentIsPushed: boolean;
		parentIsIntegrated: boolean;
	}

	const { mergedIncorrectly, isPushed, hasParent, parentIsPushed, parentIsIntegrated }: Props =
		$props();
</script>

{#snippet links(props: { url: string })}
	Please check out our
	<LinkButton
		onclick={async () => {
			openExternalUrl(props.url);
		}}
	>
		documentation
	</LinkButton> or join our <LinkButton
		onclick={async () => {
			openExternalUrl('https://discord.com/invite/MmFkmaJ42D');
		}}>Discord</LinkButton
	> for support.
{/snippet}

{#if $mergedIncorrectly}
	{#if hasParent && parentIsIntegrated}
		<InfoMessage style="warning" filled outlined={false}>
			{#snippet content()}
				<p>
					This branch appears to have been merged into an already merged branch. If this was a
					mistake, you can change the branch name and create a new pull request from the branch
					context menu.
				</p>
				<p>
					{@render links({
						url: 'https://docs.gitbutler.com/features/stacked-branches#accidentally-merged-a-stack-branch-into-an-already-merged-branch-before-it'
					})}
				</p>
			{/snippet}
		</InfoMessage>
	{:else}
		<InfoMessage style="warning" filled outlined={false}>
			{#snippet content()}
				<p>
					This branch has been merged into a branch different from your target. If this was not
					intentional you can force push and create a new pull request from the branch context menu.
				</p>
				<p>
					{@render links({
						url: 'https://docs.gitbutler.com/features/stacked-branches#accidentally-merged-a-branch-into-a-branch-before-it-not-integrated-into-mainmaster-yet'
					})}
				</p>
			{/snippet}
		</InfoMessage>
	{/if}
{/if}

{#if isPushed && hasParent && !parentIsPushed}
	<InfoMessage style="warning" filled outlined={false}>
		{#snippet content()}
			<p>
				This branch is based on a branch that has been deleted from the remote. If this was not
				intentional you can force push to recreate the branch.
			</p>
			<p>
				{@render links({ url: 'https://docs.gitbutler.com/features/stacked-branches' })}
			</p>
		{/snippet}
	</InfoMessage>
{/if}

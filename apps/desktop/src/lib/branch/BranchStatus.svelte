<script lang="ts">
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import LinkButton from '@gitbutler/ui/LinkButton.svelte';
	import type { Readable } from 'svelte/store';

	interface Props {
		mergedIncorrectly?: Readable<boolean>;
		isPushed: boolean;
		hasParent: boolean;
		parentIsPushed: boolean;
	}

	const { mergedIncorrectly, isPushed, hasParent, parentIsPushed }: Props = $props();
</script>

{#snippet links()}
	Please check out our
	<LinkButton
		onclick={async () => {
			openExternalUrl('https://docs.gitbutler.com/features/stacked-branches');
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
	<InfoMessage style="warning" filled outlined={false}>
		<svelte:fragment slot="content">
			<p>
				This branch has been merged into a branch different from your target. If this was not
				intentional you can force push and create a new pull request for this branch.
			</p>
			<p>
				{@render links()}
			</p>
		</svelte:fragment>
	</InfoMessage>
{/if}

{#if isPushed && hasParent && !parentIsPushed}
	<InfoMessage style="warning" filled outlined={false}>
		<svelte:fragment slot="content">
			<p>
				This branch is based on a branch that has been deleted from the remote. If this was not
				intentional you can force push to recreate the branch.
			</p>
			<p>
				{@render links()}
			</p>
		</svelte:fragment>
	</InfoMessage>
{/if}

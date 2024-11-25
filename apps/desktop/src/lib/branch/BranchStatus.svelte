<script lang="ts">
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import LinkButton from '@gitbutler/ui/LinkButton.svelte';
	import type { Readable } from 'svelte/store';

	interface Props {
		mergedIncorrectly?: Readable<boolean>;
	}

	const { mergedIncorrectly }: Props = $props();
</script>

{#if $mergedIncorrectly}
	<InfoMessage style="warning" filled outlined>
		<svelte:fragment slot="content">
			<p>
				It appears this branch has been merged into a branch different from your target. If this was
				not intentional you can force push and create a new pull request for this branch.
			</p>
			<p>
				Please check out our
				<LinkButton
					icon="copy-small"
					onclick={async () => {
						openExternalUrl('http://docs.gitbutler.com/development/debugging');
					}}
				>
					documentation
				</LinkButton> or join our <LinkButton
					icon="copy-small"
					onclick={async () => {
						openExternalUrl('https://discord.com/invite/MmFkmaJ42D');
					}}>Discord</LinkButton
				> for support.
			</p>
		</svelte:fragment>
	</InfoMessage>
{/if}

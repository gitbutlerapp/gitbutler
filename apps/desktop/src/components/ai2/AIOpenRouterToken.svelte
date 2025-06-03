<script lang="ts">
	import { Ai2Service } from '$lib/ai2/service';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	// const { projectId }: { projectId: string } = $props(); // Not used, so removed

	const ai2Service = getContext(Ai2Service);
	const [setOpenRouterToken] = ai2Service.setOpenRouterToken;
	const isOpenRouterTokenSet = ai2Service.isOpenRouterTokenSet;

	let openRouterToken = $state('');
	let lastValue: string | undefined = undefined;

	$effect(() => {
		if (isOpenRouterTokenSet.current.data) {
			if (openRouterToken === '') {
				openRouterToken = 'xxxx';
			}
		}
	});

	$effect(() => {
		if (lastValue !== openRouterToken && openRouterToken !== 'xxxx') {
			setOpenRouterToken({ token: openRouterToken });
			lastValue = openRouterToken;
		}
	});
</script>

<SectionCard
	labelFor="open-router-token"
	orientation="column"
	roundedBottom={false}
	roundedTop={false}
>
	{#snippet title()}
		OpenRouter Token
	{/snippet}
	{#snippet actions()}
		<Textbox
			id="open-router-token"
			value={openRouterToken}
			onchange={(value) => {
				openRouterToken = value;
			}}
			wide
		/>
	{/snippet}
</SectionCard>

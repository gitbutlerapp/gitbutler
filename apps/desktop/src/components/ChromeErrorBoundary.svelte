<script lang="ts">
	import { goto } from '$app/navigation';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import InfoMessage from '$components/InfoMessage.svelte';
	import ProjectNotFound from '$components/ProjectNotFound.svelte';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import { parseQueryError } from '$lib/error/error';
	import { Code } from '$lib/error/knownErrors';
	import { Button } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		error: unknown;
	};

	const { projectId, error }: Props = $props();

	const parsedError = $derived(parseQueryError(error));

	function isMonday() {
		const today = new Date();
		return today.getDay() === 1;
	}

	function apologiy(): string {
		if (isMonday()) {
			return 'Sorry about that. Mondays can be tough!';
		}
		return 'We apologize for the inconvenience.';
	}
</script>

{#if parsedError.code === Code.ProjectMissing}
	<ProjectNotFound {projectId} />
{:else}
	<DecorativeSplitView img={loadErrorSvg}>
		<div class="container">
			<div class="text-content">
				<h2 class="title-text text-18 text-body text-bold">Something went wrong</h2>

				<p class="description-text text-13 text-body">
					{apologiy()}
				</p>
			</div>

			<InfoMessage error={parsedError.message} style="error">
				{#snippet title()}
					{parsedError.name}
				{/snippet}
				{#snippet content()}
					An asynchronous operation failed.
				{/snippet}
			</InfoMessage>

			<div class="button-container">
				<Button type="button" style="pop" onclick={async () => await goto('/')}>Go back</Button>
			</div>
		</div>
	</DecorativeSplitView>
{/if}

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.text-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.title-text {
		color: var(--clr-scale-ntrl-30);
	}

	.button-container {
		display: flex;
		justify-content: end;
		gap: 8px;
	}

	.description-text {
		color: var(--clr-text-2);
		line-height: 1.6;
	}
</style>

<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import InexistentRepo from './errorBoundaryActions/InexistentRepo.svelte';
	import notFoundSvg from '$lib/assets/illustrations/not-found.svg?raw';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';

	const repoName = 'placeholder-repo';
	let deleteSucceeded: boolean | undefined = $state(undefined);
</script>

<DecorativeSplitView img={notFoundSvg}>
	<div class="container" data-tauri-drag-region>
		{#if deleteSucceeded === undefined}
			<div class="text-content">
				<h2 class="title-text text-18 text-body text-bold" data-tauri-drag-region>
					Can’t find "{repoName}"
				</h2>

				<p class="description-text text-13 text-body">
					Sorry, we can't find the project you're looking for.
					<br />
					It might have been removed or doesn't exist.
					<button class="check-again-btn" onclick={() => location.reload()}>Click here</button>
					to check again.
					<br />
					The current project path: <span class="code-string">/uses/estib/projects/{repoName}</span>
				</p>
			</div>

			<InexistentRepo ondeletesuccess={(deleted) => (deleteSucceeded = deleted)} />
		{/if}

		{#if deleteSucceeded === true}
			<InfoMessage filled outlined={false} style="success" icon="info">
				<svelte:fragment slot="content">Project "{repoName}" successfully deleted</svelte:fragment>
			</InfoMessage>
		{/if}

		{#if deleteSucceeded === false}
			<InfoMessage filled outlined={false} style="error" icon="info">
				<svelte:fragment slot="content">Failed to delete "{repoName}" project</svelte:fragment>
			</InfoMessage>
		{/if}

		<Spacer dotted margin={0} />
		<ProjectSwitcher />
	</div>
</DecorativeSplitView>

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
		/* margin-bottom: 12px; */
	}

	.description-text {
		color: var(--clr-text-2);
		line-height: 1.6;
	}

	.check-again-btn {
		text-decoration: underline;
	}
</style>

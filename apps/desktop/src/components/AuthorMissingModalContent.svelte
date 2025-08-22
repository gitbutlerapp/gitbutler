<script lang="ts">
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { inject } from '@gitbutler/shared/context';
	import { TestId, ModalHeader, Textbox, Button } from '@gitbutler/ui';
	import type { AuthorMissingModalState } from '$lib/state/uiState.svelte';

	type Props = {
		data: AuthorMissingModalState;
		close: () => void;
	};

	const { data, close }: Props = $props();

	const gitService = inject(GIT_SERVICE);
	const [setAuthorInfo, settingInfo] = gitService.setAuthorInfo;

	let name = $derived(data.authorName);
	let email = $derived(data.authorEmail);

	async function handleSubmit() {
		if (!name || !email) {
			return;
		}
		await setAuthorInfo({
			projectId: data.projectId,
			name,
			email
		});
		close();
	}
</script>

<div class="author-missing__wrapper">
	<ModalHeader type="warning">Missing author information in git config</ModalHeader>
	<div class="author-missing__content">
		<p class="text-13">
			Please configure your commit author details before continuing
			<br />
			This is the information that will be used when creating your commits.
		</p>

		<Textbox
			disabled={settingInfo.current.isLoading}
			placeholder="Author name"
			testId={TestId.GlobalModal_AuthorMissing_NameInput}
			bind:value={name}
			autofocus
		/>

		<Textbox
			disabled={settingInfo.current.isLoading}
			placeholder="Author email"
			testId={TestId.GlobalModal_AuthorMissing_EmailInput}
			bind:value={email}
		/>
	</div>

	<div class="author-missing__actions">
		<Button
			testId={TestId.GlobalModal_AuthorMissing_ActionButton}
			style="pop"
			onclick={handleSubmit}
			loading={settingInfo.current.isLoading}
			disabled={!name || !email}>Save</Button
		>
	</div>
</div>

<style lang="postcss">
	.author-missing__wrapper {
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.author-missing__content {
		display: flex;
		flex-direction: column;
		padding: 0 16px 16px 16px;
		gap: 16px;
	}

	.author-missing__actions {
		display: flex;
		justify-content: flex-end;
		padding: 0 16px 16px 16px;
	}
</style>

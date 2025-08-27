<script lang="ts">
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};
	const { projectId }: Props = $props();

	const gitService = inject(GIT_SERVICE);
	const uiState = inject(UI_STATE);

	// Ensure author information is present
	const authorInfo = $derived(gitService.getAuthorInfo(projectId));

	$effect(() => {
		if (authorInfo.current.data) {
			const { name, email } = authorInfo.current.data;
			if (name === null || email === null) {
				uiState.global.modal.set({
					type: 'author-missing',
					projectId,
					authorName: name ?? undefined,
					authorEmail: email ?? undefined
				});
			}
		}
	});
</script>

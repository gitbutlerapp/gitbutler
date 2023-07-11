<script lang="ts">
	import { toasts, api } from '$lib';
	import { Button, Modal } from '$lib/components';
	import { IconBookmarkFilled } from '$lib/icons';

	export let projectId: string;

	let isCreating = false;
	let timestampMs: number | undefined;

	const reset = () => {
		note = '';
		timestampMs = undefined;
	};

	export async function show(ts: number) {
		reset();
		timestampMs = ts;
		const existing = await api.bookmarks.list({
			projectId,
			range: {
				start: ts,
				end: ts + 1
			}
		});
		if (existing.length === 1) note = existing[0].note;

		modal.show();
	}

	let modal: Modal;
	let note: string;

	const createBookmark = () =>
		Promise.resolve()
			.then(() => (isCreating = true))
			.then(() =>
				api.bookmarks.upsert({
					projectId,
					note,
					timestampMs: timestampMs ?? Date.now(),
					deleted: false
				})
			)
			.then(() => {
				toasts.success('Bookmark created');
				modal.close();
			})
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to create bookmark');
			})
			.finally(() => (isCreating = false));
</script>

<Modal bind:this={modal} title="Bookmark" icon={IconBookmarkFilled}>
	<form class="flex w-full flex-col gap-2">
		<input type="submit" hidden />
		<span>Note</span>
		<!-- svelte-ignore a11y-autofocus -->
		<textarea
			on:keydown={(e) => {
				if (e.key === 'Enter' && e.metaKey) createBookmark();
			}}
			autofocus
			autocomplete="off"
			autocorrect="off"
			spellcheck="true"
			name="description"
			disabled={isCreating}
			class="h-full w-full resize-none"
			rows="6"
			bind:value={note}
		/>

		<span class="text-text-subdued"> Using hashtags to help search for bookmarks later </span>
	</form>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Close</Button>
		<Button loading={isCreating} color="purple" on:click={() => createBookmark()}>Bookmark</Button>
	</svelte:fragment>
</Modal>

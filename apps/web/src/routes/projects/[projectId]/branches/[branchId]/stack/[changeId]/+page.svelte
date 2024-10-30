<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import { getContext } from '@gitbutler/shared/context';
	import hljs from 'highlight.js';
	import { marked } from 'marked';
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { env } from '$env/dynamic/public';

	let state = 'loading';
	let patch: any = {};
	let stack: any = {};
	let key: any = '';
	let uuid: any = '';

	export let data: any;

	const authService = getContext(AuthService);

	onMount(() => {
		key = get(authService.token);
		let projectId = data.projectId;
		let branchId = data.branchId;
		let changeId = data.changeId;

		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/patch_stack/' + projectId + '/' + branchId, {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			})
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					stack = data;
					uuid = data.uuid;
					fetchPatch(data.uuid, changeId, key);
				});
		} else {
			state = 'unauthorized';
		}
	});

	function fetchPatch(uuid: string, changeId: string, key: string) {
		fetch(env.PUBLIC_APP_HOST + 'api/patch_stack/' + uuid + '/patch/' + changeId, {
			method: 'GET',
			headers: {
				'X-AUTH-TOKEN': key || ''
			}
		})
			.then(async (response) => await response.json())
			.then((data) => {
				console.log(data);
				patch = data;
				state = 'loaded';
				// wait a second
				setTimeout(() => {
					console.log('Highlighting code');
					hljs.highlightAll();
					// render markdowns
					let markdowns = document.querySelectorAll('.markdown');
					markdowns.forEach((markdown) => {
						markdown.innerHTML = marked(markdown.innerHTML);
					});
				}, 10);
			});
	}

	function createSectionPost(position: number) {
		let opts = {
			method: 'POST',
			headers: {
				'X-AUTH-TOKEN': key || '',
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({
				type: 'text',
				text: '# new section',
				position: position - 1
			})
		};
		if (key) {
			fetch(
				env.PUBLIC_APP_HOST + 'api/patch_stack/' + uuid + '/patch/' + data.changeId + '/section',
				opts
			)
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
				});
		}
	}

	function deleteSectionPost(code: string) {
		let opts = {
			method: 'DELETE',
			headers: {
				'X-AUTH-TOKEN': key || '',
				'Content-Type': 'application/json'
			}
		};
		if (key) {
			fetch(
				env.PUBLIC_APP_HOST +
					'api/patch_stack/' +
					uuid +
					'/patch/' +
					data.changeId +
					'/section/' +
					code,
				opts
			)
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
				});
		}
	}

	function deleteSection(code: string) {
		console.log('Adding section at position', code);
		deleteSectionPost(code);
		updatePatch();
	}

	function addSection(position: number) {
		console.log('Adding section at position', position);
		createSectionPost(position);
		updatePatch();
	}

	function orderSectionPatch(order: any[]) {
		let opts = {
			method: 'PATCH',
			headers: {
				'X-AUTH-TOKEN': key || '',
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({
				section_order: order
			})
		};
		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/patch_stack/' + uuid + '/patch/' + data.changeId, opts)
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
				});
		}
	}
	function moveSection(position: number, change: number) {
		console.log('Moving section at position', position, 'by', change);
		let ids = patch.sections.map((section: any) => section.identifier);
		// reorder ids array to move item in position to swap with item change off
		let temp = ids[position];
		ids[position] = ids[position + change];
		ids[position + change] = temp;
		// convert ids array to comma separated string
		orderSectionPatch(ids);
		console.log(ids);
		updatePatch();
	}

	function editSection(code: string) {
		console.log('Editing section', code);
		let editor = document.querySelector<HTMLElement>('.edit-' + code);
		if (editor) {
			editor.style.display = 'block';
			let display = document.querySelector<HTMLElement>('.display-' + code);
			if (display) {
				display.style.display = 'none';
			}
		}
	}

	function saveSection(code: string) {
		console.log('Saving section', code);
		let editor = document.querySelector<HTMLElement>('.edit-' + code);
		if (editor) {
			let text = editor.querySelector('textarea')!.value;
			let opts = {
				method: 'PATCH',
				headers: {
					'X-AUTH-TOKEN': key || '',
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({
					text: text
				})
			};
			if (key) {
				fetch(
					env.PUBLIC_APP_HOST +
						'api/patch_stack/' +
						uuid +
						'/patch/' +
						data.changeId +
						'/section/' +
						code,
					opts
				)
					.then(async (response) => await response.json())
					.then((data) => {
						console.log(data);
					});
			}
			editor.style.display = 'none';
			let display = document.querySelector<HTMLElement>('.display-' + code);
			if (display) {
				display.style.display = 'block';
				display.innerHTML = text;
				updatePatch();
			}
		}
	}
	function updatePatch() {
		setTimeout(() => {
			fetchPatch(uuid, data.changeId, key);
		}, 500);
	}
</script>

{#if state === 'loading'}
	<p>Loading...</p>
{:else if state === 'unauthorized'}
	<p>Unauthorized</p>
{:else}
	<h2>Branch: <a href="../stack">{stack.title}</a></h2>
	{#each stack.patches as stackPatch}
		<div>
			<code
				><a href="/projects/{data.projectId}/branches/{data.branchId}/stack/{stackPatch.change_id}"
					>{stackPatch.change_id.substr(0, 8)}</a
				></code
			>:
			{#if patch.change_id === stackPatch.change_id}
				<strong>{stackPatch.title}</strong>
			{:else}
				{stackPatch.title}
			{/if}
		</div>
	{/each}
	<hr />

	<h2>Patch</h2>
	<div class="columns">
		<div class="column">
			<div>Title: <strong>{patch.title}</strong></div>
			{#if patch.description}
				<div>Desc: {patch.description}</div>
			{/if}
			<div>Change Id: <code>{patch.change_id}</code></div>
			<div>Commit: <code>{patch.commit_sha}</code></div>
		</div>
		<div class="column">
			<div>Patch Version: {patch.version}</div>
			<div>Stack Position: {patch.position + 1}/{stack.stack_size}</div>
			<div>Contributors: {patch.contributors}</div>
			<div>
				Additions: {patch.statistics.lines - patch.statistics.deletions}, Deletions: {patch
					.statistics.deletions}, Files: {patch.statistics.file_count}
			</div>
		</div>
	</div>

	<div class="columns">
		<div class="column outline">
			<h3>Outline</h3>
			<div class="sections">
				{#each patch.sections as section}
					{#if section.section_type === 'diff'}
						<div><a href="#section-{section.id}">{section.new_path}</a></div>
					{:else}
						<div><a href="#section-{section.id}">{section.title}</a></div>
					{/if}
				{/each}
			</div>
		</div>
		<div class="column">
			<div class="patch">
				{#each patch.sections as section}
					<div id="section-{section.id}">
						{#if section.section_type === 'diff'}
							<div class="right">
								<button type="button" class="action" on:click={() => addSection(section.position)}
									>add</button
								>
								[<button
									type="button"
									class="action"
									on:click={() => moveSection(section.position, -1)}>up</button
								>
								<button
									type="button"
									class="action"
									on:click={() => moveSection(section.position, 1)}>down</button
								>]
							</div>
							<div>
								<strong>{section.new_path}</strong>
							</div>
							<div><pre><code>{section.diff_patch}</code></pre></div>
						{:else}
							<div class="right">
								<button type="button" class="action" on:click={() => addSection(section.position)}
									>add</button
								>
								[
								<button type="button" class="action" on:click={() => editSection(section.code)}
									>edit</button
								>] [
								<button type="button" class="action" on:click={() => deleteSection(section.code)}
									>del</button
								>] [
								<button
									type="button"
									class="action"
									on:click={() => moveSection(section.position, -1)}>up</button
								>
								<button
									type="button"
									class="action"
									on:click={() => moveSection(section.position, 1)}>down</button
								>
								]
							</div>
							<div class="editor edit-{section.code}">
								<textarea class="editing">{section.data.text}</textarea>
								<button type="button" on:click={() => saveSection(section.code)}>Save</button>
							</div>
							<div class="markdown display-{section.code}">{section.data.text}</div>
						{/if}
					</div>
				{/each}
				<div class="right">
					<button type="button" class="action" on:click={() => addSection(patch.sections.length)}
						>add</button
					>
				</div>
			</div>
		</div>
	</div>
{/if}
<link
	rel="stylesheet"
	href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.10.0/styles/default.min.css"
/>

<style>
	hr {
		margin: 1rem 0;
	}
	code {
		background-color: #f4f4f4;
		padding: 0.2rem 0.4rem;
		border-radius: 4px;
	}
	strong {
		font-weight: bold;
	}
	.columns {
		display: flex;
	}
	.column {
		flex: 1;
		padding: 1rem;
	}
	.outline {
		max-width: 250px;
	}
	.right {
		display: flex;
		flex-direction: row;
		justify-content: flex-end;
		gap: 5px;
		color: #888;
	}
	.action {
		cursor: pointer;
		color: #999;
	}
	.sections {
		display: flex;
		flex-direction: column;
		gap: 5px;
	}
	.editing {
		width: 100%;
		height: 100px;
		font-family: monospace;
		font-size: large;
	}
	.editor {
		display: none;
	}
	.patch {
		background-color: #ffffff;
		border-radius: 10px;
		padding: 10px 20px;
	}
</style>

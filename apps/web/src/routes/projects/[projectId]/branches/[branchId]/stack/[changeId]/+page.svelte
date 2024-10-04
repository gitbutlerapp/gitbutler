<script lang="ts">
	import DiffPatch from '$lib/components/DiffPatch.svelte';
	import DiffPatchArray from '$lib/components/DiffPatchArray.svelte';
	import { marked } from 'marked';
	import { onMount } from 'svelte';
	import Gravatar from 'svelte-gravatar';
	import { env } from '$env/dynamic/public';

	let state = 'loading';
	let patch: any = {};
	let stack: any = {};
	let status: any = {};
	let chats: any = [];
	let key: any = '';
	let uuid: any = '';

	export let data: any;

	onMount(() => {
		key = localStorage.getItem('gb_access_token');

		let projectId = data.projectId;
		let branchId = data.branchId;
		let changeId = data.changeId;

		// scroll chatWindow to bottom

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
					getPatchStatus();
					fetchAndUpdateChat();
				});
		} else {
			state = 'unauthorized';
		}
	});

	function scrollToBottom() {
		let chatWindow = document.querySelector<HTMLElement>('.chatWindow');
		if (chatWindow) {
			chatWindow.scrollTop = chatWindow.scrollHeight;
		}
	}

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

	function getPatchStatus() {
		//GET        /api/patch_stack/:project_id/:branch_id/patch_status
		fetch(
			env.PUBLIC_APP_HOST +
				'api/patch_stack/' +
				data.projectId +
				'/' +
				data.branchId +
				'/patch_status',
			{
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			}
		)
			.then(async (response) => await response.json())
			.then((data) => {
				status = data;
				console.log('patch status');
				console.log(data);
			});
	}

	function fetchAndUpdateChat() {
		fetch(env.PUBLIC_APP_HOST + 'api/chat_messages/' + data.projectId + '/chats/' + data.changeId, {
			method: 'GET',
			headers: {
				'X-AUTH-TOKEN': key || ''
			}
		})
			.then(async (response) => await response.json())
			.then((data) => {
				console.log(data);
				setTimeout(() => {
					chats = data;
					setTimeout(() => {
						scrollToBottom();
					}, 150); // I don't know how to DOM in Svelte, but it takes a second
				}, 50); // I don't know how to DOM in Svelte, but it takes a second
			});
	}

	function createChatMessage() {
		let chatBox = document.querySelector<HTMLElement>('.chatBox');
		if (chatBox) {
			// check if this is an issue
			var checkbox = document.getElementById('issue');
			let is_issue = false;
			if ((checkbox as HTMLInputElement).checked) {
				is_issue = true;
			}

			let text = chatBox.querySelector('textarea')!.value;
			let params: {
				chat: string;
				change_id: any;
				range?: string;
				diff_path?: string;
				diff_sha?: string;
				issue?: boolean;
			} = {
				chat: text,
				change_id: data.changeId,
				issue: is_issue
			};
			if (commentRange.length > 0) {
				params.range = commentRange;
				params.diff_path = diffPath;
				params.diff_sha = diffSha;
			}
			let opts = {
				method: 'POST',
				headers: {
					'X-AUTH-TOKEN': key || '',
					'Content-Type': 'application/json'
				},
				body: JSON.stringify(params)
			};
			if (key) {
				fetch(
					env.PUBLIC_APP_HOST + 'api/chat_messages/' + data.projectId + '/branch/' + data.branchId,
					opts
				)
					.then(async (response) => await response.json())
					.then((data) => {
						chatBox.querySelector('textarea')!.value = '';
						fetchAndUpdateChat();
						console.log(data);
					});
			}
		}
	}

	function signOff(signoff: boolean) {
		let opts = {
			method: 'PATCH',
			headers: {
				'X-AUTH-TOKEN': key || '',
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({
				sign_off: signoff
			})
		};
		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/patch_stack/' + uuid + '/patch/' + data.changeId, opts)
				.then(async (response) => await response.json())
				.then((data) => {
					console.log('sign off', data);
					getPatchStatus();
				});
		}
	}

	function resolveIssue(uuid: string) {
		//:project_id/chat/:chat_uuid
		console.log('Resolving issue', uuid);
		let opts = {
			method: 'PATCH',
			headers: {
				'X-AUTH-TOKEN': key || '',
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({
				resolved: true
			})
		};
		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/chat_messages/' + data.projectId + '/chat/' + uuid, opts)
				.then(async (response) => await response.json())
				.then((data) => {
					console.log('resolved', data);
					fetchAndUpdateChat();
				});
		}
	}

	let commentRange: string = '';
	let diffPath: string = '';
	let diffSha: string = '';

	function rangeSelect(range: string, diff_path: string, diff_sha: string) {
		console.log('range selected', range, diff_path, diff_sha);
		commentRange = range;
		diffPath = diff_path;
		diffSha = diff_sha;
	}
</script>

{#if state === 'loading'}
	<p>Loading...</p>
{:else if state === 'unauthorized'}
	<p>Unauthorized</p>
{:else}
	<div class="columns">
		<div class="column patchArea">
			<h3>Patch Series: <a href="../stack">{stack.title}</a></h3>
			{#each stack.patches as stackPatch}
				<div>
					<code
						><a
							rel="external"
							href="/projects/{data.projectId}/branches/{data.branchId}/stack/{stackPatch.change_id}"
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

			<h3>Patch</h3>
			<div class="columns">
				<div class="column">
					<div>Title: <strong>{patch.title}</strong></div>
					{#if patch.description}
						<div>Desc: {patch.description}</div>
					{/if}
					<div>Change Id: <code>{patch.change_id.substr(0, 13)}</code></div>
					<div>Commit SHA: <code>{patch.commit_sha.substr(0, 10)}</code></div>
					<div>Patch Version: {patch.version}</div>
					<div>Series Position: {patch.position + 1}/{stack.stack_size}</div>
					<div>
						Contributors:
						{#each patch.contributors as email}
							<Gravatar {email} size={20} />
						{/each}
					</div>
					<div>
						Additions: {patch.statistics.lines - patch.statistics.deletions}, Deletions: {patch
							.statistics.deletions}, Files: {patch.statistics.file_count}
					</div>
				</div>
				<div class="column">
					{#if patch.review.viewed.length > 0}
						<div>
							<div class="title">Viewed:</div>
							{#each patch.review.viewed as email}
								<Gravatar {email} size={20} />
							{/each}
						</div>
					{/if}

					{#if patch.review.signed_off.length > 0}
						<div>
							<div class="title">Signed Off:</div>
							{#each patch.review.signed_off as email}
								<Gravatar {email} size={20} />
							{/each}
						</div>
					{/if}

					{#if patch.review.length > 0}
						<div>
							<div class="title">Rejected:</div>
							{#each patch.review.rejected as email}
								<Gravatar {email} size={20} />
							{/each}
						</div>
					{/if}

					<h3>Your sign off status</h3>
					{#if status[data.changeId]}
						{#if status[data.changeId].last_signoff}
							<div class="signoff">You signed off on this patch</div>
						{/if}
						{#if !status[data.changeId].last_reviewed}
							<div class="rejected">You have not reviewed this patch</div>
						{:else if !status[data.changeId].last_signoff}
							<div class="rejected">You have rejected this patch</div>
						{/if}
					{/if}
					<div>
						{#if !status[data.changeId].last_signoff}
							<button class="button" on:click={() => signOff(true)}>Sign Off</button>
						{/if}
						{#if status[data.changeId].last_signoff || !status[data.changeId].last_reviewed}
							<button class="button" on:click={() => signOff(false)}>Reject</button>
						{/if}
					</div>
				</div>
			</div>

			<hr />

			<div class="patch">
				{#each patch.sections as section}
					<div id="section-{section.id}">
						{#if section.section_type === 'diff'}
							<div class="right">
								<button class="action" on:click={() => addSection(section.position)}>add</button>
								[<button class="action" on:click={() => moveSection(section.position, -1)}
									>up</button
								>
								<button class="action" on:click={() => moveSection(section.position, 1)}
									>down</button
								>]
							</div>
							<div class="filePath">
								{section.new_path}
							</div>
							<div>
								<DiffPatch
									onRangeSelect={(range, diff_path, diff_sha) =>
										rangeSelect(range, diff_path, diff_sha)}
									diffPath={section.new_path}
									diffSha={section.diff_sha}
									diff={section.diff_patch}
								/>
							</div>
						{:else}
							<div class="right">
								<button class="action" on:click={() => addSection(section.position)}>add</button>
								[
								<button class="action" on:click={() => editSection(section.code)}>edit</button>] [
								<button class="action" on:click={() => deleteSection(section.code)}>del</button>] [
								<button class="action" on:click={() => moveSection(section.position, -1)}>up</button
								>
								<button class="action" on:click={() => moveSection(section.position, 1)}
									>down</button
								>
								]
							</div>
							<div class="editor edit-{section.code}">
								<textarea class="editing">{section.data.text}</textarea>
								<button on:click={() => saveSection(section.code)}>Save</button>
							</div>
							<div class="markdown display-{section.code}">{section.data.text}</div>
						{/if}
					</div>
				{/each}
				<div class="right">
					<button class="action" on:click={() => addSection(patch.sections.length)}>add</button>
				</div>
			</div>
		</div>
		<div class="column chatArea">
			<h3>Chat</h3>
			<div class="chatWindow">
				{#each chats as chat}
					<div class="chatEntry {chat.issue ? 'issue' : ''} {chat.resolved ? 'resolved' : ''}">
						<div class="chatHeader">
							<div class="avatar">
								<Gravatar email={chat.user.email} size={20} />
							</div>
							<div>{chat.created_at}</div>
						</div>
						{#if chat.diff_patch_array}
							<div>
								<div class="diffPath">{chat.diff_path}</div>
								<!-- {chat.diff_sha} -->
								<DiffPatchArray diffArray={chat.diff_patch_array} />
							</div>
						{/if}
						<div class="chatComment">{chat.comment}</div>
						{#if chat.issue}
							{#if chat.resolved}
								<div class="right">resolved</div>
							{:else}
								<button class="action" on:click={() => resolveIssue(chat.uuid)}>resolve</button>
							{/if}
						{/if}
					</div>
				{/each}
			</div>
			<div class="chatBox">
				<div class="input">
					<div class="messageBox">
						<textarea></textarea>
					</div>
					<div class="chatOptions">
						<div class="issueOptions">
							<input type="checkbox" id="issue" name="issue" value="issue" />
							<div>issue</div>
						</div>
						<button class="actionChat" on:click={() => createChatMessage()}>send</button>
					</div>
				</div>
			</div>
		</div>
	</div>
{/if}

<style>
	.actionChat {
		cursor: pointer;
		color: #ffffff;
		padding: 5px 10px;
		background-color: var(--clr-theme-pop-element);
		border-radius: 5px;
	}
	.issueOptions {
		display: flex;
		flex-direction: row;
		gap: 5px;
		color: #626262;
		/* justify vertically */
		align-items: center;
	}
	.chatOptions {
		padding: 4px;
		display: flex;
		flex-direction: row;
		justify-content: space-between;
	}
	.patchArea {
		max-width: 800px;
		overflow-x: scroll;
	}
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
	.patch-diff {
		font-family: monospace;
		font-size: small;
	}
	h3 {
		margin-bottom: 0.5rem;
		font-weight: bold;
	}
	.button {
		background-color: #f4f4f4;
		border: 1px solid #ccc;
		padding: 5px;
	}
	.chatArea {
		width: 600px;
		min-width: 600px;
	}
	.chatWindow {
		border: 1px solid #ccc;
		border-radius: 5px;
		padding: 5px;
		margin: 5px 0;
		max-height: 600px;
		height: 600px;
		overflow-y: scroll;
	}
	.chatEntry {
		border: 1px solid #ccc;
		border-radius: 5px;
		background-color: #f4f4f4;
		padding: 5px;
		margin: 5px 0;
	}
	.chatEntry.issue {
		background-color: #ffeef0;
	}
	.chatEntry.issue.resolved {
		background-color: #eeeeee;
	}
	.chatHeader {
		display: flex;
		flex-direction: row;
		justify-content: space-between;
		font-size: small;
	}
	.chatComment {
		margin-top: 5px;
		background-color: #ffffff;
		padding: 5px;
	}
	.chatBox {
		border: 1px solid #ccc;
		border-radius: 5px;
		background-color: #f4f4f4;
		padding: 5px;
		margin: 5px 0;
	}
	.chatBox textarea {
		width: 100%;
		height: 40px;
		border-radius: 5px;
		font-family: monospace;
		font-size: large;
		padding: 5px;
	}
	.avatar {
		border-radius: 50%;
		overflow: hidden;
	}

	.filePath {
		font-family: monospace;
		font-weight: bold;
		font-size: 1.4em;
		margin-bottom: 10px;
	}
	.signoff {
		background-color: #e6ffed;
		padding: 5px;
		border-radius: 5px;
	}
	.rejected {
		background-color: #ffeef0;
		padding: 5px;
		border-radius: 5px;
	}
	.title {
		min-width: 100px;
		display: inline-block;
		border-right: 1px solid #eee;
	}
	.diffPath {
		font-family: monospace;
		font-weight: bold;
		font-size: 1.1em;
		margin-top: 10px;
		margin-bottom: 6px;
	}
</style>

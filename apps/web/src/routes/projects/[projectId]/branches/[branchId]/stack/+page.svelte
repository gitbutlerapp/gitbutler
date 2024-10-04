<script lang="ts">
	import moment from 'moment';
	import { onMount } from 'svelte';
	import Gravatar from 'svelte-gravatar';
	import { env } from '$env/dynamic/public';

	// load moment
	let state = 'loading';
	let stackData: any = {};

	export let data: any;

	onMount(() => {
		let key = localStorage.getItem('gb_access_token');
		let projectId = data.projectId;
		let branchId = data.branchId;

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
					stackData = data;
					state = 'loaded';
					// moment all the .dtime elements
					// wait a second
					setTimeout(() => {
						let dtime = document.querySelectorAll('.dtime');
						dtime.forEach((element) => {
							console.log(element.innerHTML);
							element.innerHTML = moment(element.innerHTML).fromNow();
						});
					}, 100);
				});
		} else {
			state = 'unauthorized';
		}
	});
</script>

{#if state === 'loading'}
	<p>Loading...</p>
{:else if state === 'unauthorized'}
	<p>Unauthorized</p>
{:else}
	<div><a href="/projects/{data.projectId}">project</a></div>

	<h2>Patch Stack</h2>
	<div class="columns">
		<div class="column">
			Title: <strong>{stackData.title}</strong><br />
			Branch: <code>{stackData.branch_id}</code><br />
			Stack UUID: <code>{stackData.uuid}</code><br />
			Updated: <span class="dtime">{stackData.created_at}</span><br />
		</div>
		<div class="column">
			Stack Size: {stackData.stack_size}<br />
			{#if stackData.version > 1}
				Version: <a href="/projects/{data.projectId}/branches/{stackData.branch_id}/versions"
					>{stackData.version}</a
				><br />
			{:else}
				Version: {stackData.version}<br />
			{/if}
			Contributors: {stackData.contributors}<br />
		</div>
	</div>

	<hr />

	<h2>Patches</h2>
	{#each stackData.patches as patch}
		<div class="patch">
			{#if patch.review.rejected.length > 0}
				<div class="patchHeader rejected">X</div>
			{:else if patch.review.signed_off.length > 0}
				<div class="patchHeader signoff">✓</div>
			{:else}
				<div class="patchHeader unreviewed">?</div>
			{/if}
			<div class="columns patchData">
				<div class="column">
					<div>Title: <strong>{patch.title}</strong></div>
					<div>
						Change Id: <code><a href="./stack/{patch.change_id}">{patch.change_id}</a></code>
					</div>
					<div>Commit: <code>{patch.commit_sha}</code></div>
					<div>Version: {patch.version}</div>
					<div><strong>Files:</strong></div>
					{#each patch.statistics.files as file}
						<div><code>{file}</code></div>
					{/each}
				</div>
				<div class="column">
					<div>Created: <span class="dtime">{patch.created_at}</span></div>
					<div>Contributors: {patch.contributors}</div>
					<div>
						Additions: {patch.statistics.lines - patch.statistics.deletions}, Deletions: {patch
							.statistics.deletions}, Files: {patch.statistics.file_count}
					</div>
					<hr />
					<div>
						Viewed:
						{#each patch.review.viewed as email}
							<Gravatar {email} size={20} />
						{/each}
					</div>
					<div>
						Signed Off:
						{#each patch.review.signed_off as email}
							<Gravatar {email} size={20} />
						{/each}
					</div>
					<div>
						Rejected:
						{#each patch.review.rejected as email}
							<Gravatar {email} size={20} />
						{/each}
					</div>
				</div>
			</div>
		</div>
	{/each}
{/if}

<style>
	hr {
		margin: 10px 0;
	}
	h2 {
		font-size: 1.5rem;
	}
	.columns {
		display: flex;
	}
	.column {
		flex: 1;
	}
	.column div {
		margin: 4px 0;
	}
	.patch {
		background-color: var(--clr-bg-1-muted);
		border: 1px solid #ccc;
		margin: 10px 0;
		border-radius: 10px;
	}
	.patchData {
		padding: 15px 20px;
	}
	.patchHeader {
		padding: 5px;
		border-radius: 5px 5px 0 0;
	}
	.rejected {
		background-color: rgb(224, 92, 92);
		color: white;
	}
	.signoff {
		background-color: rgb(77, 219, 77);
		color: white;
	}
	.unreviewed {
		background-color: rgb(215, 215, 144);
		color: black;
	}
</style>

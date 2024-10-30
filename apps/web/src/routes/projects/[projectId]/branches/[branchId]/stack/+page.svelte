<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import { getContext } from '@gitbutler/shared/context';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { env } from '$env/dynamic/public';

	dayjs.extend(relativeTime);

	let state = 'loading';
	let stackData: any = {};

	export let data: any;

	const authService = getContext(AuthService);

	onMount(() => {
		const key = get(authService.token);
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
					let dtime = document.querySelectorAll('.dtime');
					dtime.forEach((element) => {
						console.log(element.innerHTML);
						element.innerHTML = dayjs(element.innerHTML).fromNow();
					});
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

	<h1>Branch</h1>
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
		<div class="columns patch">
			<div class="column">
				<div>Title: <strong>{patch.title}</strong></div>
				<div>Change Id: <code><a href="./stack/{patch.change_id}">{patch.change_id}</a></code></div>
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
		background-color: #fff;
		border: 1px solid #ccc;
		padding: 15px 20px;
		margin: 10px 0;
		border-radius: 10px;
	}
</style>

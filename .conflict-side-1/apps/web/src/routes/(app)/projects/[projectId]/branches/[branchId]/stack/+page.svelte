<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { onMount } from 'svelte';

	const httpClient = getContext(HttpClient);

	dayjs.extend(relativeTime);

	let pageState = $state('loading');
	let stackData: any = $state({});

	interface Props {
		data: any;
	}

	let { data }: Props = $props();

	onMount(async () => {
		let key = localStorage.getItem('gb_access_token');
		let projectId = data.projectId;
		let branchId = data.branchId;

		if (key) {
			const stackData = await httpClient.get('patch_stack/' + projectId + '/' + branchId, {
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			});
			console.log(stackData);
			pageState = 'loaded';
			let dtime = document.querySelectorAll('.dtime');
			dtime.forEach((element) => {
				console.log(element.innerHTML);
				element.innerHTML = dayjs(element.innerHTML).fromNow();
			});
		} else {
			pageState = 'unauthorized';
		}
	});
</script>

{#if pageState === 'loading'}
	<p>Loading...</p>
{:else if pageState === 'unauthorized'}
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

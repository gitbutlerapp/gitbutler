<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import { getContext } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { env } from '$env/dynamic/public';

	let state = 'loading';
	let timeline: any = {};
	let patchStacks: any = {};
	let project: any = {};
	let projectId: string;

	export let data: any;

	const authService = getContext(AuthService);

	function createPatchStack(branch: string, sha: string) {
		const key = get(authService.token);

		let opts = {
			method: 'POST',
			headers: {
				'X-AUTH-TOKEN': key || '',
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({
				project_id: projectId,
				branch_id: branch,
				oplog_sha: sha
			})
		};
		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/patch_stack', opts)
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					goto('/projects/' + projectId + '/branches/' + data.branch_id + '/stack');
				});
		} else {
			state = 'unauthorized';
		}
	}

	onMount(() => {
		const key = get(authService.token);
		projectId = data.projectId;
		console.log(projectId);
		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/projects/' + projectId, {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			})
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					project = data;
					state = 'loaded';
				});

			fetch(env.PUBLIC_APP_HOST + 'api/patch_stack/' + projectId, {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			})
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					patchStacks = data;
				});

			fetch(env.PUBLIC_APP_HOST + 'api/timeline/' + projectId, {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			})
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					timeline = data;
					state = 'loaded';
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
	<h2>Project</h2>
	<div>{project.name}</div>
	<div class="columns">
		<div class="column">
			<h2>Branches</h2>
			{#each patchStacks as stack}
				<div>
					{stack.title}<br />
					<a href="/projects/{data.projectId}/branches/{stack.branch_id}/stack">{stack.branch_id}</a
					><br />
					{stack.stack_size} patches<br />
					{stack.contributors}<br />
					v{stack.version}<br />
					{stack.created_at}<br />
				</div>
				<hr />
			{/each}
		</div>
		<div class="column">
			<h2>Timeline</h2>
			{#each timeline as event}
				<div class="event">
					<pre>{event.sha}</pre>
					<div>{event.time}</div>
					<pre>{event.message}</pre>
					<div>{event.trailers}</div>
					{#if Object.keys(event.files).length > 0}
						<h3>Branches</h3>
						{#each Object.keys(event.files) as branch}
							{#if event.branch_data.branches[branch]}
								<h3>Branch {event.branch_data.branches[branch].name}</h3>
								<button type="button" on:click={() => createPatchStack(branch, event.sha)}
									>Create Patch Stack</button
								>
								{#each Object.keys(event.files[branch]) as file}
									<li>{file} ({event.files[branch][file]})</li>
								{/each}
							{/if}
						{/each}
					{/if}
				</div>
				<hr />
			{/each}
		</div>
	</div>
{/if}

<style>
	h2 {
		margin-bottom: 1rem;
	}
	.event {
		margin-bottom: 1rem;
	}
	hr {
		margin: 1rem 0;
	}
	.columns {
		display: flex;
	}
	.column {
		flex: 1;
		padding: 1rem;
	}
</style>

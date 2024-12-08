<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { onMount } from 'svelte';
	import { env } from '$env/dynamic/public';

	dayjs.extend(relativeTime);

	let pageState = $state('loading');
	let stackData: any = $state({});

	interface Props {
		data: any;
	}

	let { data }: Props = $props();

	onMount(() => {
		let key = localStorage.getItem('gb_access_token');
		let projectId = data.projectId;
		let branchId = data.branchId;

		if (key) {
			fetch(
				env.PUBLIC_APP_HOST +
					'api/patch_stack/' +
					projectId +
					'?branch_id=' +
					branchId +
					'&status=all',
				{
					method: 'GET',
					headers: {
						'X-AUTH-TOKEN': key || ''
					}
				}
			)
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					stackData = data;
					pageState = 'loaded';
					let dtime = document.querySelectorAll('.dtime');
					dtime.forEach((element) => {
						console.log(element.innerHTML);
						element.innerHTML = dayjs(element.innerHTML).fromNow();
					});
				});
		} else {
			pageState = 'unauthorized';
		}
	});
</script>

{#if pageState === 'loading'}
	<p></p>
{:else if pageState === 'unauthorized'}
	<p>Unauthorized</p>
{:else}
	<div><a href="/projects/{data.projectId}">project</a></div>

	<h1>Stack History</h1>

	{#each stackData as stack}
		<div class="columns">
			<div class="column">
				Title: <strong>{stack.title}</strong><br />
				Branch: <code>{stack.branch_id}</code><br />
				Stack UUID: <code>{stack.uuid}</code><br />
				Updated: <span class="dtime">{stack.created_at}</span><br />
			</div>
			<div class="column">
				Stack Size: {stack.stack_size}<br />
				Version: {stack.version}<br />
				Contributors: {stack.contributors}<br />
			</div>
		</div>
		<div>
			Patches:
			<ul>
				{#each stack.patches as patch}
					<li>
						<code style="background-color:#{patch.change_id.substr(0, 6)}"
							>{patch.change_id.substr(0, 6)}</code
						>:
						<code style="background-color:#{patch.commit_sha.substr(0, 6)}"
							>{patch.commit_sha.substr(0, 6)}</code
						>: {patch.title} : v{patch.version}
					</li>
				{/each}
			</ul>
		</div>
		<hr />
	{/each}
{/if}

<style>
	hr {
		margin: 10px 0;
	}
	.columns {
		display: flex;
	}
	.column {
		flex: 1;
	}
</style>

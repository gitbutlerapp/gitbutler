<script lang="ts">
	type Props = {
		title: string;
		slug: string;
		picture: string;
		date: string;
		commits: number;
		files: number;
		additions: number;
		subtractions: number;
		commit_titles: string[];
	};

	let {
		title,
		slug,
		picture,
		date,
		commit_titles,
		commits,
		files,
		additions,
		subtractions
	}: Props = $props();
	// limit commit_titles to 3 max
	commit_titles = commit_titles.slice(0, 3);
</script>

<div
	style="display: flex; flex-direction: column; height: 100%; width: 100%; justify-content: space-between; letter-spacing: -0.02em; font-weight: 700; background: white; padding: 35px; background-image: linear-gradient(to bottom, #f1f1f1, #dbf4ff);"
>
	<div style="display: flex; flex-direction: row; justify-content: space-between;">
		<div style="display: flex; flex-direction: column;">
			<div style="display: flex; gap: 12px; margin-bottom: 10px;">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					style="margin-top: 10px; margin-right: 10px"
					width="40"
					height="40"
					viewBox="0 0 23 24"
					fill="none"
				>
					<path d="M0 24V0L11.4819 10.5091L23 0V24L11.4819 13.5273L0 24Z" fill="black"></path>
				</svg>
				<div style="font-weight: 100; font-size: 40px; color: #666; padding-bottom: 10px;">
					{slug}
				</div>
			</div>
			<div
				style="display: flex; color: #000000; font-size: 60px; width: 800px; height: 180px; overflow: hidden; text-overflow: ellipsis; padding-bottom: 10px;"
			>
				{title}
			</div>
		</div>
		<div style="display: flex;">
			<img style="border-radius: 50%;" width="250px" height="250px" alt="avatar" src={picture} />
		</div>
	</div>
	<div style="display: flex; flex-direction: column;">
		{#each commit_titles as commit_title}
			<div
				style="display: flex; color: #666; font-size: 30px; font-weight: 100; padding: 10px; margin-right: 10px;"
			>
				- {commit_title.substring(0, 70)}
			</div>
		{/each}
	</div>
	<div
		style="display: flex; align-items: center; justify-content: space-between; color: #57606a; font-size: 40px; font-weight: 100;"
	>
		<div style="display: flex; gap: 50px;">
			<span style="color: #333;">{date}</span>
			{#if commits > 1}
				<span>{commits} commits</span>
			{:else}
				<span>{commits} commit</span>
			{/if}
			<span style="color: #888;">{files} files</span>
		</div>
		<div style="display: flex; font-size: 60px; gap: 25px; font-weight: 800;">
			<span style="color: #28a745;">+{additions}</span>
			<span style="color: #d73a49;">-{subtractions}</span>
		</div>
	</div>
</div>

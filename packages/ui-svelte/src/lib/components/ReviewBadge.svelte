<script lang="ts">
	import Badge from "$components/Badge.svelte";
	import { getForgeLogo } from "$lib/utils/getForgeLogo";

	interface Props {
		type: string | undefined;
		forge?: string;
		status?: "open" | "closed" | "draft" | "merged" | "unknown";
		number: number;
		title?: string;
		testId?: string;
	}

	const { type, forge, status, number, title, testId }: Props = $props();

	const reviewUnit = $derived(type === "MR" ? "MR" : "PR");
	const reviewSymbol = $derived(reviewUnit === "MR" ? "!" : "#");

	// Prefer the forge-specific logo; fall back to inferring from the review unit
	// for callers that don't pass a forge name.
	const icon = $derived(
		forge ? getForgeLogo(forge) : type === "MR" ? "gitlab" : type === "PR" ? "github" : undefined,
	);
	const id = $derived(`${reviewSymbol}${number}`);

	function getBadgeStyle(status: Props["status"]): "safe" | "danger" | "purple" | "gray" {
		switch (status) {
			case "open":
				return "safe";
			case "closed":
				return "danger";
			case "merged":
				return "purple";
			default:
				return "gray";
		}
	}

	const badgeDetails = $derived.by(() => {
		if (title) {
			return title;
		}

		switch (status) {
			case "open":
				return `${reviewUnit} ${id} is open`;
			case "closed":
				return `${reviewUnit} ${id} is closed`;
			case "draft":
				return `${reviewUnit} ${id} is a draft`;
			case "merged":
				return `${reviewUnit} ${id} is merged`;
			default:
				return `${reviewUnit} ${id}`;
		}
	});
</script>

<Badge
	{testId}
	tooltip={badgeDetails}
	style={getBadgeStyle(status)}
	kind="soft"
	{icon}
	reversedDirection
>
	{#if status === "draft"}
		Draft {reviewUnit}
	{:else}
		{reviewUnit} {id}
	{/if}
</Badge>

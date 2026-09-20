<script module lang="ts">
	export type PrStatusInfoType = "loading" | "open" | "merged" | "closed" | "draft";
</script>

<script lang="ts">
	import Badge from "$components/Badge.svelte";
	import { type IconName } from "$lib/icons/names";
	import type { ComponentColorType } from "$lib/utils/colorTypes";

	interface Props {
		status: PrStatusInfoType;
	}

	type StatusInfo = {
		text: string;
		icon: IconName;
		style?: ComponentColorType;
	};

	const { status }: Props = $props();

	const prStatusInfo: StatusInfo = $derived.by(() => {
		switch (status) {
			case "loading":
				return { text: "Loading...", icon: "spinner", style: "gray" };
			case "merged":
				return { text: "Merged", icon: "pr-tick", style: "purple" };
			case "closed":
				return { text: "Closed", icon: "pr-cross", style: "danger" };
			case "draft":
				return { text: "Draft", icon: "pr-draft", style: "gray" };
			default:
				return { text: "Open", icon: "pr", style: "safe" };
		}
	});
</script>

<Badge style={prStatusInfo.style} kind="soft" reversedDirection size="icon" icon={prStatusInfo.icon}
	>{prStatusInfo.text}</Badge
>

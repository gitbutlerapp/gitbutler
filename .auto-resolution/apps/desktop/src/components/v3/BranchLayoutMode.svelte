<script module lang="ts">
	enum LayoutEnum {
		Multi = 'multi',
		Single = 'single',
		Vertical = 'vertical'
	}
	export type Layout = LayoutEnum[keyof LayoutEnum];
</script>

<script lang="ts">
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';

	type Props = {
		mode: Layout;
	};

	let { mode = $bindable() }: Props = $props();
</script>

<SegmentControl
	defaultIndex={mode === LayoutEnum.Multi ? 0 : mode === LayoutEnum.Single ? 1 : 2}
	onselect={(id) => (mode = id as Layout)}
	size="small"
>
	<Segment id={LayoutEnum.Multi} icon="horz-columns" tooltip="Rail View" tooltipPosition="top" />
	<Segment id={LayoutEnum.Single} icon="single-view" tooltip="Wide View" tooltipPosition="top" />
	<Segment
		id={LayoutEnum.Vertical}
		icon="vert-columns"
		tooltip="Stacked view"
		tooltipPosition="top"
	/>
</SegmentControl>

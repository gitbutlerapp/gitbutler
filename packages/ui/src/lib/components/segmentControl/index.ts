import SegmentItem from '$components/segmentControl/Segment.svelte';
import SegmentControlRoot from '$components/segmentControl/SegmentControl.svelte';

type SegmentControlType = typeof SegmentControlRoot & {
	Item: typeof SegmentItem;
};

const SegmentControl = Object.assign(SegmentControlRoot, {
	Item: SegmentItem
}) as SegmentControlType;

export { SegmentControl };
export { SegmentItem };

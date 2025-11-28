import SectionCard from '$components/section/SectionCard.svelte';
import SectionRoot from '$components/section/SectionRoot.svelte';

type SectionType = typeof SectionRoot & {
	Card: typeof SectionCard;
};

const Section = Object.assign(SectionRoot, {
	Card: SectionCard
}) as SectionType;

export { Section };
export { SectionCard };

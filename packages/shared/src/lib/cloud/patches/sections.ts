import { get, type Readable } from 'svelte/store';
import type { CloudPatchService } from '$lib/cloud/patches/service';

export class PatchSectionsService {
	readonly canMoveSection: Readable<boolean>;

	constructor(private readonly cloudPatchService: CloudPatchService) {
		this.canMoveSection = cloudPatchService.canUpdatePatch;
	}

	/**
	 * Moves the section to wards the page top
	 *
	 * This means that it will move the section towards the index 0 of the array
	 */
	moveSectionUp(identifier: string) {
		if (!get(this.canMoveSection)) return;

		const patch = get(this.cloudPatchService.patch).value;
		if (!patch) return;

		const identifiers = patch.sections.map((section) => section.identifier);

		const sectionIndex = identifiers.findIndex(
			(listedIdentifier) => listedIdentifier === identifier
		);

		swap(identifiers, sectionIndex, sectionIndex - 1);

		this.cloudPatchService.update({ sectionOrder: identifiers });
	}

	/**
	 * Moves the section to wards the page bottom
	 *
	 * This means that it will move the section away from the index 0 of the array
	 */
	moveSectionDown(identifier: string) {
		if (!get(this.canMoveSection)) return;

		const patch = get(this.cloudPatchService.patch).value;
		if (!patch) return;

		const identifiers = patch.sections.map((section) => section.identifier);

		const sectionIndex = identifiers.findIndex(
			(listedIdentifier) => listedIdentifier === identifier
		);

		swap(identifiers, sectionIndex, sectionIndex + 1);

		this.cloudPatchService.update({ sectionOrder: identifiers });
	}
}

/**
 * Swaps two elements in an array
 *
 * Returns false if the indicies were out of bounds
 */
function swap(input: unknown[], a: number, b: number): boolean {
	const indiciesTooSmall = a < 0 || b < 0;
	const indiciesTooLarge = a >= input.length || b >= input.length;
	if (indiciesTooLarge || indiciesTooSmall) return false;

	[input[a], input[b]] = [input[b], input[a]];

	return true;
}

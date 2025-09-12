import type { ModeService } from '$lib/mode/modeService';

export interface EditPatchParams {
	modeService: ModeService | undefined;
	commitId: string;
	stackId: string;
	projectId: string;
}

/**
 * Utility function to enter edit mode for a specific commit.
 * Centralizes the logic for entering patch edit mode across components.
 */
export async function editPatch(params: EditPatchParams): Promise<void> {
	if (!params.modeService) {
		console.warn('Mode service not available for edit patch operation');
		return;
	}

	if (!params.commitId || !params.stackId || !params.projectId) {
		console.warn('Missing required parameters for edit patch operation', params);
		return;
	}

	try {
		await params.modeService.enterEditMode({
			commitId: params.commitId,
			stackId: params.stackId,
			projectId: params.projectId
		});
	} catch (error) {
		console.error('Failed to enter edit mode:', error);
		throw error;
	}
}

/**
 * Helper function to check if edit patch functionality is available
 */
export function canEditPatch(modeService: ModeService | undefined): boolean {
	return modeService !== undefined;
}

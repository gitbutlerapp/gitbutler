import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { Section } from '$lib/branches/types';

const patchSectionsAdapter = createEntityAdapter<Section, Section['id']>({
	selectId: (patchSection: Section) => patchSection.id
});

const patchSectionsSlice = createSlice({
	name: 'patchSectionSections',
	initialState: patchSectionsAdapter.getInitialState(),
	reducers: {
		addPatchSection: patchSectionsAdapter.addOne,
		addPatchSections: patchSectionsAdapter.addMany,
		removePatchSection: patchSectionsAdapter.removeOne,
		removePatchSections: patchSectionsAdapter.removeMany,
		upsertPatchSection: patchSectionsAdapter.upsertOne,
		upsertPatchSections: patchSectionsAdapter.upsertMany
	}
});

export const patchSectionsReducer = patchSectionsSlice.reducer;

export const patchSectionsSelectors = patchSectionsAdapter.getSelectors();
export const {
	addPatchSection,
	addPatchSections,
	removePatchSection,
	removePatchSections,
	upsertPatchSection,
	upsertPatchSections
} = patchSectionsSlice.actions;

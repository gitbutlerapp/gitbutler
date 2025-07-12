import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
import { SETTINGS, type Settings } from '$lib/settings/userSettings';
import { UiState } from '$lib/state/uiState.svelte';
import { TestId } from '$lib/testing/testIds';
import { HttpClient } from '@gitbutler/shared/network/httpClient';
import { UploadsService } from '@gitbutler/shared/uploads/uploadsService';
import { readable, writable } from 'svelte/store';
import '../../src/styles/styles.css';
import '@gitbutler/ui/main.css';

describe('CommitMesageEditor.cy.ts', () => {
	const httpClient = new HttpClient(window.fetch, 'https://www.example.com', writable(''));
	const settings = writable({} as Settings);
	it('playground', () => {
		const context = new Map();
		const uiState = new UiState(readable({ ids: [], entities: {} }), () => {});
		context.set(UiState, uiState);
		context.set(UploadsService, new UploadsService(httpClient));
		context.set(SETTINGS, settings);

		const mountResult = cy.mount(MessageEditor, {
			props: {
				projectId: '1234',
				initialValue: 'Hello world!',
				placeholder: 'text goes here',
				testId: TestId.EditCommitMessageBox
			} as const,
			context
		});
		mountResult
			.then(async ({ component }) => {
				const comp = component as MessageEditor;
				return await comp.getPlaintext();
			})
			.should('eq', 'Hello world!');
		cy.getByTestId(TestId.EditCommitMessageBox).should('exist').click().type('new text!');
	});
});
